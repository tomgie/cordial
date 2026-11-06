//! The context that each handler gets.

use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use cordial_cache::Cache;
use cordial_gateway::ShardHandle;
use cordial_http::Client;
use cordial_model::gateway::event::GatewayEvent;
use cordial_model::id::{ApplicationId, GuildId};
use tokio::sync::watch;

use crate::collector::{Collectors, EventCollector};

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
    pub(crate) collectors: Collectors,
    pub(crate) shutdown: watch::Sender<bool>,
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
                collectors: Collectors::default(),
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
        let count = u32::try_from(shards.len())
            .ok()
            .filter(|count| *count > 0)?;
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
        tokio::time::timeout(timeout, collector.next())
            .await
            .ok()
            .flatten()
    }

    /// Collects the events of type `E` that match the filter, until the collector is dropped.
    pub fn collect<E>(&self, filter: impl FnMut(&E) -> bool + Send + 'static) -> EventCollector<E>
    where
        E: GatewayEvent + Clone + Send + 'static,
    {
        self.shared.collectors.add(filter)
    }

    fn read_shards(&self) -> std::sync::RwLockReadGuard<'_, Vec<ShardHandle>> {
        self.shared
            .shards
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
