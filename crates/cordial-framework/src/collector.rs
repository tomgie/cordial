//! Collectors: wait for events that match a filter.
//!
//! A collector serves a short, live conversation, for example "wait for the next button
//! press on this message". For state that must stay across restarts, put it in the
//! `custom_id` and use the component router.

use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cordial_model::gateway::event::{Event, GatewayEvent};
use tokio::sync::mpsc;

type Filter = Box<dyn FnMut(&Event) -> bool + Send>;

struct Waiter {
    id: u64,
    filter: Filter,
    sender: mpsc::UnboundedSender<Arc<Event>>,
}

/// The waiting collectors of a bot.
#[derive(Default)]
pub(crate) struct Collectors {
    inner: Arc<CollectorsInner>,
}

#[derive(Default)]
struct CollectorsInner {
    next_id: AtomicU64,
    waiters: Mutex<Vec<Waiter>>,
}

impl CollectorsInner {
    fn waiters(&self) -> std::sync::MutexGuard<'_, Vec<Waiter>> {
        self.waiters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Collectors {
    pub(crate) fn add<E>(
        &self,
        mut filter: impl FnMut(&E) -> bool + Send + 'static,
    ) -> EventCollector<E>
    where
        E: GatewayEvent + Clone + Send + 'static,
    {
        let (sender, receiver) = mpsc::unbounded_channel();
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let filter: Filter =
            Box::new(move |event| E::from_event_ref(event).is_some_and(&mut filter));
        self.inner.waiters().push(Waiter { id, filter, sender });

        EventCollector {
            id,
            collectors: Arc::clone(&self.inner),
            receiver,
            kind: PhantomData,
        }
    }

    /// Gives an event to the matching collectors. Tells if a collector took the event.
    pub(crate) fn dispatch(&self, event: &Arc<Event>) -> bool {
        let mut waiters = self.inner.waiters();
        waiters.retain(|waiter| !waiter.sender.is_closed());

        let mut taken = false;
        for waiter in waiters.iter_mut() {
            if (waiter.filter)(event) && waiter.sender.send(Arc::clone(event)).is_ok() {
                taken = true;
            }
        }
        taken
    }
}

/// The events of one collector. Drop it to stop the collection.
pub struct EventCollector<E> {
    id: u64,
    collectors: Arc<CollectorsInner>,
    receiver: mpsc::UnboundedReceiver<Arc<Event>>,
    kind: PhantomData<fn() -> E>,
}

impl<E> std::fmt::Debug for EventCollector<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("EventCollector")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl<E: GatewayEvent + Clone> EventCollector<E> {
    /// Waits for the next matching event. Gives `None` when the bot stops.
    pub async fn next(&mut self) -> Option<E> {
        loop {
            let event = self.receiver.recv().await?;
            if let Some(payload) = E::from_event_ref(&event) {
                return Some(payload.clone());
            }
        }
    }

    /// Waits for the next matching event, up to a timeout.
    pub async fn next_timeout(&mut self, timeout: Duration) -> Option<E> {
        tokio::time::timeout(timeout, self.next())
            .await
            .ok()
            .flatten()
    }
}

impl<E> Drop for EventCollector<E> {
    fn drop(&mut self) {
        self.collectors
            .waiters()
            .retain(|waiter| waiter.id != self.id);
    }
}
