//! The shard manager: it starts and supervises many shards.

use std::collections::HashMap;
use std::sync::Arc;

use cordial_http::Client;
use cordial_model::gateway::{EventTypes, GatewayBotInfo, Intents, UpdatePresence};
use tokio::sync::mpsc;

use crate::compression::Compression;
use crate::core::Session;
use crate::queue::{IdentifyQueue, LocalIdentifyQueue};
use crate::shard::{CloseMode, Shard, ShardConfig, ShardHandle, ShardMessage};

/// The number of session starts that the manager keeps in reserve. Discord resets the bot
/// token when a bot goes over the daily limit.
pub const SESSION_START_RESERVE: u32 = 5;

/// An error when the shard manager starts.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ManagerError {
    /// The request to `GET /gateway/bot` failed.
    #[error("cannot get the gateway data: {0}")]
    Http(#[from] cordial_http::Error),
    /// The daily session start limit is too low for the shards.
    #[error(
        "only {remaining} session starts are left today (reset in {reset_after_ms} ms); \
         {needed} shards need to start"
    )]
    SessionStartLimit {
        /// The session starts left.
        remaining: u32,
        /// The shards that need a new session.
        needed: u32,
        /// The time until the limit resets, in milliseconds.
        reset_after_ms: u64,
    },
    /// The shard settings are not valid.
    #[error("the shard settings are not valid: {0}")]
    InvalidShards(String),
}

/// Which shards the manager runs.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ShardRange {
    /// The number of shards that Discord recommends, all of them.
    Recommended,
    /// A fixed number of shards, all of them.
    All {
        /// The shard count.
        count: u32,
    },
    /// Some shards of a fixed count, for bots that run shards in many processes.
    Some {
        /// The shard IDs that this process runs.
        ids: Vec<u32>,
        /// The total shard count.
        count: u32,
    },
}

/// Settings and builder of a [`ShardManager`].
#[derive(Debug)]
pub struct ShardManagerBuilder {
    http: Client,
    intents: Intents,
    range: ShardRange,
    compression: Compression,
    presence: Option<UpdatePresence>,
    event_types: EventTypes,
    identify_queue: Option<Arc<dyn IdentifyQueue>>,
    sessions: HashMap<u32, Session>,
    large_threshold: Option<u8>,
    event_buffer: usize,
}

impl ShardManagerBuilder {
    /// Makes a builder. The HTTP client must have the bot token.
    #[must_use]
    pub fn new(http: Client, intents: Intents) -> Self {
        Self {
            http,
            intents,
            range: ShardRange::Recommended,
            compression: Compression::default(),
            presence: None,
            event_types: EventTypes::all(),
            identify_queue: None,
            sessions: HashMap::new(),
            large_threshold: None,
            event_buffer: 4096,
        }
    }

    /// Sets which shards run.
    #[must_use]
    pub fn shards(mut self, range: ShardRange) -> Self {
        self.range = range;
        self
    }

    /// Sets the transport compression.
    #[must_use]
    pub fn compression(mut self, compression: Compression) -> Self {
        self.compression = compression;
        self
    }

    /// Sets the initial presence of all shards.
    #[must_use]
    pub fn presence(mut self, presence: UpdatePresence) -> Self {
        self.presence = Some(presence);
        self
    }

    /// Sets the event types to decode.
    #[must_use]
    pub fn event_types(mut self, event_types: EventTypes) -> Self {
        self.event_types = event_types;
        self
    }

    /// Sets the identify queue. The default is a local queue with the `max_concurrency`
    /// of the bot.
    #[must_use]
    pub fn identify_queue(mut self, queue: Arc<dyn IdentifyQueue>) -> Self {
        self.identify_queue = Some(queue);
        self
    }

    /// Gives sessions to resume, for example from a process that stopped with
    /// [`ShardManager::close_resumable`].
    #[must_use]
    pub fn resume_sessions(mut self, sessions: HashMap<u32, Session>) -> Self {
        self.sessions = sessions;
        self
    }

    /// Sets the member count above which Discord does not send offline members.
    #[must_use]
    pub fn large_threshold(mut self, threshold: u8) -> Self {
        self.large_threshold = Some(threshold);
        self
    }

    /// Sets the capacity of the event channel.
    #[must_use]
    pub fn event_buffer(mut self, capacity: usize) -> Self {
        self.event_buffer = capacity.max(1);
        self
    }

    /// Gets the gateway data, checks the session start limit, and starts the shards.
    ///
    /// # Errors
    ///
    /// Gives an error when the gateway data is not available, the shard settings are not
    /// valid, or too few session starts are left.
    pub async fn start(self) -> Result<ShardManager, ManagerError> {
        let token = self.http.token().cloned().ok_or_else(|| {
            ManagerError::InvalidShards("the HTTP client has no token".to_owned())
        })?;
        let info: GatewayBotInfo = self.http.gateway_bot().await?;

        let (ids, count) = match self.range {
            ShardRange::Recommended => ((0..info.shards).collect(), info.shards),
            ShardRange::All { count } => ((0..count).collect(), count),
            ShardRange::Some { ids, count } => (ids, count),
        };
        if count == 0 || ids.iter().any(|id| *id >= count) {
            return Err(ManagerError::InvalidShards(format!(
                "the shard IDs must be lower than the shard count {count}"
            )));
        }

        let needed = u32::try_from(
            ids.iter()
                .filter(|id| !self.sessions.contains_key(id))
                .count(),
        )
        .unwrap_or(u32::MAX);
        let limit = info.session_start_limit;
        if needed.saturating_add(SESSION_START_RESERVE) > limit.remaining {
            return Err(ManagerError::SessionStartLimit {
                remaining: limit.remaining,
                needed,
                reset_after_ms: limit.reset_after,
            });
        }

        let queue = self
            .identify_queue
            .unwrap_or_else(|| Arc::new(LocalIdentifyQueue::new(limit.max_concurrency)));
        let (events, receiver) = mpsc::channel(self.event_buffer);
        let mut sessions = self.sessions;

        let mut shards = Vec::with_capacity(ids.len());
        for id in ids {
            let mut config = ShardConfig::new(token.clone(), self.intents)
                .shard(id, count)
                .gateway_url(info.url.clone())
                .compression(self.compression)
                .event_types(self.event_types)
                .identify_queue(Arc::clone(&queue));
            config.presence.clone_from(&self.presence);
            config.large_threshold = self.large_threshold;
            config.event_buffer = self.event_buffer;
            if let Some(session) = sessions.remove(&id) {
                config = config.resume(session);
            }
            shards.push(Shard::start_with_sender(config, events.clone()));
        }

        tracing::info!(
            shards = shards.len(),
            count,
            max_concurrency = limit.max_concurrency,
            "shards started"
        );
        Ok(ShardManager {
            shards,
            count,
            events: receiver,
        })
    }
}

/// Runs and supervises many shards, and merges their events.
#[derive(Debug)]
pub struct ShardManager {
    shards: Vec<ShardHandle>,
    count: u32,
    events: mpsc::Receiver<ShardMessage>,
}

impl ShardManager {
    /// Makes a builder.
    #[must_use]
    pub fn builder(http: Client, intents: Intents) -> ShardManagerBuilder {
        ShardManagerBuilder::new(http, intents)
    }

    /// Gives the next event of any shard. Gives `None` when all shards have stopped.
    pub async fn next_event(&mut self) -> Option<ShardMessage> {
        self.events.recv().await
    }

    /// Gives the handles of the shards of this process.
    #[must_use]
    pub fn shards(&self) -> &[ShardHandle] {
        &self.shards
    }

    /// Gives the total shard count.
    #[must_use]
    pub fn shard_count(&self) -> u32 {
        self.count
    }

    /// Gives the handle of the shard of a guild, if this process runs it.
    #[must_use]
    pub fn shard_for_guild(&self, guild_id: cordial_model::id::GuildId) -> Option<&ShardHandle> {
        let id = shard_id_for_guild(guild_id, self.count);
        self.shards.iter().find(|shard| shard.id() == id)
    }

    /// Closes all shards and ends their sessions.
    pub async fn close(&self) {
        for shard in &self.shards {
            shard.close(CloseMode::End).await;
        }
    }

    /// Closes all shards and keeps their sessions, so that a new process can resume them.
    pub async fn close_resumable(&self) -> HashMap<u32, Session> {
        let mut sessions = HashMap::new();
        for shard in &self.shards {
            if let Some(session) = shard.close(CloseMode::Resumable).await {
                sessions.insert(shard.id(), session);
            }
        }
        sessions
    }
}

/// Gives the shard of a guild: `(guild_id >> 22) % shard_count`.
#[must_use]
pub fn shard_id_for_guild(guild_id: cordial_model::id::GuildId, shard_count: u32) -> u32 {
    let shard = (guild_id.get() >> 22) % u64::from(shard_count.max(1));
    u32::try_from(shard).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shard_formula() {
        let guild = cordial_model::id::GuildId::new_const(41_771_983_423_143_937);

        assert_eq!(shard_id_for_guild(guild, 1), 0);
        assert_eq!(
            shard_id_for_guild(guild, 16),
            u32::try_from((41_771_983_423_143_937_u64 >> 22) % 16).unwrap()
        );
    }
}
