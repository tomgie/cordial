//! Collectors: wait for events that match a filter.
//!
//! A collector serves a short, live conversation, for example "wait for the next button
//! press on this message". For state that must stay across restarts, put it in the
//! `custom_id` and use the component router.

use std::marker::PhantomData;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
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
    /// One dispatch at a time. The gateway loop and HTTP interactions both dispatch.
    dispatching: Mutex<()>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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
        lock(&self.inner.waiters).push(Waiter { id, filter, sender });

        EventCollector {
            id,
            collectors: Arc::clone(&self.inner),
            receiver,
            closed: false,
            kind: PhantomData,
        }
    }

    /// Gives an event to the matching collectors. Tells if a collector took the event.
    ///
    /// The filters run without the waiter lock, so that a filter can start or stop a
    /// collector. A filter that panics stops only its own collector.
    pub(crate) fn dispatch(&self, event: &Arc<Event>) -> bool {
        let _turn = lock(&self.inner.dispatching);
        let mut waiters = std::mem::take(&mut *lock(&self.inner.waiters));

        let mut taken = false;
        waiters.retain_mut(|waiter| {
            if waiter.sender.is_closed() {
                return false;
            }
            match std::panic::catch_unwind(AssertUnwindSafe(|| (waiter.filter)(event))) {
                Ok(true) => {
                    taken |= waiter.sender.send(Arc::clone(event)).is_ok();
                    true
                }
                Ok(false) => true,
                Err(_) => {
                    tracing::error!(
                        event = event.name(),
                        "a collector filter panicked; the collector stops"
                    );
                    false
                }
            }
        });

        // Collectors that started while the filters ran are in the list now.
        let mut current = lock(&self.inner.waiters);
        waiters.append(&mut current);
        *current = waiters;
        taken
    }
}

/// The events of one collector. Drop it to stop the collection.
///
/// Events that the collector took but did not give out are lost when it is dropped. For an
/// interaction, nobody answers it then.
pub struct EventCollector<E> {
    id: u64,
    collectors: Arc<CollectorsInner>,
    receiver: mpsc::UnboundedReceiver<Arc<Event>>,
    closed: bool,
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

impl<E> EventCollector<E> {
    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        // The dispatch lock makes sure that no dispatch sends to this collector after this.
        let _turn = lock(&self.collectors.dispatching);
        lock(&self.collectors.waiters).retain(|waiter| waiter.id != self.id);
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
        match tokio::time::timeout(timeout, self.next()).await {
            Ok(event) => event,
            Err(_) => self.close_and_drain(),
        }
    }

    /// Stops the collection and gives an event that arrived before the stop, if any.
    pub(crate) fn close_and_drain(&mut self) -> Option<E> {
        self.close();
        while let Ok(event) = self.receiver.try_recv() {
            if let Some(payload) = E::from_event_ref(&event) {
                return Some(payload.clone());
            }
        }
        None
    }
}

impl<E> Drop for EventCollector<E> {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use cordial_model::gateway::event::MessageCreate;

    use super::*;

    fn message_event() -> Arc<Event> {
        let raw = serde_json::value::RawValue::from_string(
            serde_json::json!({
                "id": "9", "channel_id": "5",
                "author": {"id": "6", "username": "u", "discriminator": "0", "avatar": null},
                "content": "hello", "timestamp": "2026-01-01T00:00:00+00:00",
                "edited_timestamp": null, "tts": false, "mention_everyone": false,
                "mentions": [], "mention_roles": [], "attachments": [], "embeds": [],
                "pinned": false, "type": 0
            })
            .to_string(),
        )
        .unwrap();
        Arc::new(Event::decode("MESSAGE_CREATE", &raw).unwrap())
    }

    #[test]
    fn panicking_filter_stops_only_its_collector() {
        let collectors = Collectors::default();
        let _bad = collectors.add(|_: &MessageCreate| panic!("bad filter"));
        let mut good = collectors.add(|_: &MessageCreate| true);

        assert!(collectors.dispatch(&message_event()));
        assert!(collectors.dispatch(&message_event()));

        assert_eq!(lock(&collectors.inner.waiters).len(), 1);
        assert!(good.close_and_drain().is_some());
    }

    #[test]
    fn closed_collector_gives_the_event_that_arrived_before() {
        let collectors = Collectors::default();
        let mut collector = collectors.add(|_: &MessageCreate| true);

        collectors.dispatch(&message_event());

        assert!(collector.close_and_drain().is_some());
        assert!(
            !collectors.dispatch(&message_event()),
            "a closed collector takes nothing"
        );
    }
}
