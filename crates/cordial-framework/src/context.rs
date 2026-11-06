//! The context that each handler gets.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

use cordial_cache::Cache;
use cordial_gateway::ShardHandle;
use cordial_http::Client;
use cordial_model::gateway::event::{EventTypes, GatewayEvent};
use cordial_model::id::{ApplicationId, GuildId, InteractionId};
use dashmap::DashMap;
use tokio::sync::watch;

use crate::collector::{Collectors, EventCollector};
use crate::interaction::ResponseTracker;

/// An interaction token is valid for 15 minutes. A tracker that nobody took is old after it.
const TRACKER_LIFETIME: Duration = Duration::from_secs(15 * 60);

/// The context of a handler: the HTTP client, the cache, the shards, and the user state.
///
/// It is cheap to clone.
pub struct Context<S> {
    pub(crate) shared: Arc<Shared<S>>,
}

pub(crate) struct Shared<S> {
    pub(crate) http: Client,
    pub(crate) cache: Option<Cache>,
    pub(crate) state: S,
    pub(crate) application_id: OnceLock<ApplicationId>,
    pub(crate) shards: RwLock<Vec<ShardHandle>>,
    /// The total shard count of the bot, also of shards in other processes.
    pub(crate) shard_count: AtomicU32,
    /// The event types that the gateway sends. Collectors of other types never fire.
    pub(crate) event_types: OnceLock<EventTypes>,
    pub(crate) collectors: Collectors,
    /// The response trackers of interactions that a collector took. The collector context
    /// uses the same tracker as the HTTP source, which waits for the response.
    pub(crate) trackers: DashMap<InteractionId, (Instant, Arc<ResponseTracker>)>,
    pub(crate) shutdown: watch::Sender<bool>,
}

impl<S> Shared<S> {
    /// Keeps the tracker of an interaction for a collector.
    pub(crate) fn keep_tracker(
        &self,
        interaction_id: InteractionId,
        tracker: Arc<ResponseTracker>,
    ) {
        let now = Instant::now();
        if self.trackers.len() > 1000 {
            self.trackers
                .retain(|_, (created, _)| now.duration_since(*created) < TRACKER_LIFETIME);
        }
        self.trackers.insert(interaction_id, (now, tracker));
    }

    /// Takes the tracker of an interaction, or makes a new one.
    pub(crate) fn take_tracker(&self, interaction_id: InteractionId) -> Arc<ResponseTracker> {
        self.trackers
            .remove(&interaction_id)
            .map_or_else(ResponseTracker::new, |(_, (_, tracker))| tracker)
    }
}

impl<S> Clone for Context<S> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<S> std::fmt::Debug for Context<S> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Context")
            .field("application_id", &self.shared.application_id.get())
            .finish_non_exhaustive()
    }
}

impl<S: Send + Sync + 'static> Context<S> {
    pub(crate) fn new(http: Client, cache: Option<Cache>, state: S) -> Self {
        let (shutdown, _) = watch::channel(false);
        Self {
            shared: Arc::new(Shared {
                http,
                cache,
                state,
                application_id: OnceLock::new(),
                shards: RwLock::new(Vec::new()),
                shard_count: AtomicU32::new(0),
                event_types: OnceLock::new(),
                collectors: Collectors::default(),
                trackers: DashMap::new(),
                shutdown,
            }),
        }
    }

    /// Gives the HTTP client.
    #[must_use]
    pub fn http(&self) -> &Client {
        &self.shared.http
    }

    /// Gives the cache, when the bot has one.
    #[must_use]
    pub fn cache(&self) -> Option<&Cache> {
        self.shared.cache.as_ref()
    }

    /// Gives the user state.
    #[must_use]
    pub fn state(&self) -> &S {
        &self.shared.state
    }

    /// Gives the application ID. It is known after the bot starts.
    ///
    /// # Panics
    ///
    /// Panics when the bot has not started. Handlers run only after the start.
    #[must_use]
    pub fn application_id(&self) -> ApplicationId {
        *self
            .shared
            .application_id
            .get()
            .expect("the application ID is set when the bot starts")
    }

    /// Gives the handle of a shard.
    #[must_use]
    pub fn shard(&self, shard_id: u32) -> Option<ShardHandle> {
        self.read_shards()
            .iter()
            .find(|shard| shard.id() == shard_id)
            .cloned()
    }

    /// Gives the handle of the shard that has a guild.
    #[must_use]
    pub fn shard_for_guild(&self, guild_id: GuildId) -> Option<ShardHandle> {
        let shards = self.read_shards();
        // The total count, not the number of shards in this process: the guild can be on a
        // shard of another process.
        let count = match self.shared.shard_count.load(Ordering::Relaxed) {
            0 => u32::try_from(shards.len())
                .ok()
                .filter(|count| *count > 0)?,
            count => count,
        };
        let shard_id = cordial_gateway::shard_id_for_guild(guild_id, count);
        shards.iter().find(|shard| shard.id() == shard_id).cloned()
    }

    /// Gives the handles of all shards of this process.
    #[must_use]
    pub fn shards(&self) -> Vec<ShardHandle> {
        self.read_shards().clone()
    }

    /// Stops the bot. `Bot::run` returns after the shards close.
    pub fn shutdown(&self) {
        self.shared.shutdown.send_replace(true);
    }

    /// Waits for the next event of type `E` that matches the filter.
    ///
    /// Gives `None` at the timeout.
    pub async fn wait_for<E>(
        &self,
        filter: impl FnMut(&E) -> bool + Send + 'static,
        timeout: Duration,
    ) -> Option<E>
    where
        E: GatewayEvent + Clone + Send + 'static,
    {
        let mut collector = self.collect(filter);
        match tokio::time::timeout(timeout, collector.next()).await {
            Ok(event) => event,
            // An event can arrive between the timeout and the end of the collector. Take it,
            // because the router skips an interaction that a collector took.
            Err(_) => collector.close_and_drain(),
        }
    }

    /// Collects the events of type `E` that match the filter, until the collector is dropped.
    pub fn collect<E>(&self, filter: impl FnMut(&E) -> bool + Send + 'static) -> EventCollector<E>
    where
        E: GatewayEvent + Clone + Send + 'static,
    {
        if let Some(types) = self.shared.event_types.get()
            && !types.contains(E::TYPE)
        {
            tracing::warn!(
                event = ?E::TYPE,
                "a collector waits for an event type that the bot does not request; add it \
                 with BotBuilder::extra_events"
            );
        }
        self.shared.collectors.add(filter)
    }

    fn read_shards(&self) -> std::sync::RwLockReadGuard<'_, Vec<ShardHandle>> {
        self.shared
            .shards
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
