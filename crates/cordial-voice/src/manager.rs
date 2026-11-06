//! The voice manager: joins and leaves voice channels through the main gateway.
//!
//! To join, the bot sends Update Voice State (opcode 4) on the main gateway. Discord then
//! sends `VOICE_STATE_UPDATE` (with the session ID) and `VOICE_SERVER_UPDATE` (with the
//! token and the endpoint). The manager gets these events from [`VoiceManager::handle_event`]
//! and starts the [`Call`].

use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use cordial_gateway::{Command, ShardHandle, shard_id_for_guild};
use cordial_model::gateway::UpdateVoiceState;
use cordial_model::gateway::event::Event;
use cordial_model::id::{ChannelId, GuildId, UserId};
use tokio::sync::oneshot;

use crate::call::{Call, CallConfig, CallError, lock};
use crate::core::ConnectionInfo;
use crate::e2ee::{E2eeSession, NoE2ee};

/// A future that a [`VoiceGateway`] gives.
pub type GatewayFuture<'a> = Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;

/// The main gateway connection that sends Update Voice State (opcode 4).
///
/// It is implemented for one [`ShardHandle`] and for a list of shard handles (one for each
/// shard, routed by guild).
pub trait VoiceGateway: Send + Sync + 'static {
    /// Sends Update Voice State for the guild of `update`.
    fn update_voice_state(&self, update: UpdateVoiceState) -> GatewayFuture<'_>;
}

impl VoiceGateway for ShardHandle {
    fn update_voice_state(&self, update: UpdateVoiceState) -> GatewayFuture<'_> {
        Box::pin(async move {
            self.send(Command::UpdateVoiceState(update))
                .await
                .map_err(|error| error.to_string())
        })
    }
}

impl VoiceGateway for Vec<ShardHandle> {
    fn update_voice_state(&self, update: UpdateVoiceState) -> GatewayFuture<'_> {
        Box::pin(async move {
            let count = u32::try_from(self.len()).map_err(|error| error.to_string())?;
            let shard_id = shard_id_for_guild(update.guild_id, count.max(1));
            let shard = self
                .iter()
                .find(|shard| shard.id() == shard_id)
                .ok_or_else(|| format!("no handle for shard {shard_id}"))?;
            shard
                .send(Command::UpdateVoiceState(update))
                .await
                .map_err(|error| error.to_string())
        })
    }
}

/// An error of a join or a leave.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum JoinError {
    /// The manager does not know the user ID of the bot. Give it `READY` or set it.
    #[error("the user ID of the bot is not known; give the READY event to the manager")]
    UnknownUser,
    /// The main gateway did not accept Update Voice State.
    #[error("the gateway did not send Update Voice State: {0}")]
    Gateway(String),
    /// The voice events did not arrive in time. The channel can be full, or the bot has no
    /// permission to connect.
    #[error("the voice events did not arrive in time")]
    Timeout,
    /// The bot left the channel while the join waited.
    #[error("the bot left the voice channel")]
    Left,
    /// The voice connection failed.
    #[error(transparent)]
    Call(#[from] CallError),
}

/// A factory of E2EE sessions: one for each call.
pub type E2eeFactory = dyn Fn() -> Box<dyn E2eeSession> + Send + Sync;

#[derive(Default)]
struct GuildVoice {
    session_id: Option<String>,
    channel_id: Option<ChannelId>,
    token: Option<String>,
    endpoint: Option<String>,
    waiters: Vec<oneshot::Sender<ConnectionInfo>>,
    call: Option<Call>,
}

struct ManagerInner {
    gateway: Box<dyn VoiceGateway>,
    user_id: Mutex<Option<UserId>>,
    guilds: Mutex<HashMap<GuildId, GuildVoice>>,
    config: CallConfig,
    e2ee: Box<E2eeFactory>,
}

/// Joins and leaves voice channels, and keeps one [`Call`] for each guild. It is cheap to
/// clone.
///
/// Give every gateway event to [`VoiceManager::handle_event`]. The manager needs `READY`
/// (for the user ID), `VOICE_STATE_UPDATE`, and `VOICE_SERVER_UPDATE`. The intent
/// `GUILD_VOICE_STATES` is necessary.
///
/// ```no_run
/// # async fn run(shard: cordial_gateway::ShardHandle, guild_id: cordial_model::id::GuildId,
/// #     channel_id: cordial_model::id::ChannelId) -> Result<(), Box<dyn std::error::Error>> {
/// use cordial_voice::{OggOpusSource, VoiceManager};
///
/// let manager = VoiceManager::new(shard);
/// // In the event loop: manager.handle_event(&event);
/// let call = manager.join(guild_id, channel_id).await?;
/// call.play(OggOpusSource::open("song.ogg")?);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct VoiceManager {
    inner: Arc<ManagerInner>,
}

impl fmt::Debug for VoiceManager {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VoiceManager")
            .field("user_id", &*lock(&self.inner.user_id))
            .field("config", &self.inner.config)
            .finish_non_exhaustive()
    }
}

impl VoiceManager {
    /// Makes a manager with the default call settings and no E2EE.
    ///
    /// Discord needs DAVE E2EE in voice channels (close code 4017). Set an E2EE session
    /// factory with [`VoiceManager::with_e2ee`].
    #[must_use]
    pub fn new(gateway: impl VoiceGateway) -> Self {
        Self::with_e2ee(gateway, CallConfig::default(), || {
            Box::new(NoE2ee::default())
        })
    }

    /// Makes a manager with call settings and a factory of E2EE sessions.
    #[must_use]
    pub fn with_e2ee(
        gateway: impl VoiceGateway,
        config: CallConfig,
        e2ee: impl Fn() -> Box<dyn E2eeSession> + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner: Arc::new(ManagerInner {
                gateway: Box::new(gateway),
                user_id: Mutex::new(None),
                guilds: Mutex::new(HashMap::new()),
                config,
                e2ee: Box::new(e2ee),
            }),
        }
    }

    /// Sets the user ID of the bot. `READY` also sets it.
    pub fn set_user_id(&self, user_id: UserId) {
        *lock(&self.inner.user_id) = Some(user_id);
    }

    /// Gives the call of a guild.
    #[must_use]
    pub fn call(&self, guild_id: GuildId) -> Option<Call> {
        lock(&self.inner.guilds)
            .get(&guild_id)
            .and_then(|guild| guild.call.clone())
    }

    /// Handles a gateway event. Give every event; the manager ignores the events it does not
    /// need.
    pub fn handle_event(&self, event: &Event) {
        match event {
            Event::Ready(ready) => self.set_user_id(ready.user.id),
            Event::VoiceStateUpdate(update) => {
                let own = *lock(&self.inner.user_id);
                if Some(update.user_id) != own {
                    return;
                }
                let Some(guild_id) = update.guild_id else {
                    return;
                };
                self.on_voice_state(guild_id, update.channel_id, &update.session_id);
            }
            Event::VoiceServerUpdate(update) => {
                self.on_voice_server(update.guild_id, &update.token, update.endpoint.as_deref());
            }
            _ => {}
        }
    }

    fn on_voice_state(&self, guild_id: GuildId, channel_id: Option<ChannelId>, session_id: &str) {
        let mut guilds = lock(&self.inner.guilds);
        let Some(channel_id) = channel_id else {
            // The bot left, or it was kicked or moved out of all channels.
            if let Some(guild) = guilds.remove(&guild_id)
                && let Some(call) = guild.call
            {
                call.disconnect();
            }
            return;
        };

        let guild = guilds.entry(guild_id).or_default();
        guild.session_id = Some(session_id.to_owned());
        guild.channel_id = Some(channel_id);
        if let Some(call) = &guild.call {
            call.set_channel(channel_id);
        }
        self.complete(guild_id, guild);
    }

    fn on_voice_server(&self, guild_id: GuildId, token: &str, endpoint: Option<&str>) {
        let mut guilds = lock(&self.inner.guilds);
        let guild = guilds.entry(guild_id).or_default();
        guild.token = Some(token.to_owned());
        guild.endpoint = endpoint.map(ToOwned::to_owned);
        if endpoint.is_none() {
            // The voice server is being moved. A new VOICE_SERVER_UPDATE follows.
            if let Some(call) = &guild.call {
                call.server_lost();
            }
            return;
        }
        self.complete(guild_id, guild);
    }

    /// Gives the connection data to waiting joins, and moves an existing call when its
    /// session, token, or endpoint changed.
    fn complete(&self, guild_id: GuildId, guild: &mut GuildVoice) {
        let Some(user_id) = *lock(&self.inner.user_id) else {
            return;
        };
        let (Some(session_id), Some(channel_id), Some(token), Some(endpoint)) = (
            guild.session_id.clone(),
            guild.channel_id,
            guild.token.clone(),
            guild.endpoint.clone(),
        ) else {
            return;
        };
        let info = ConnectionInfo::new(guild_id, channel_id, user_id, session_id, token, endpoint);

        if let Some(call) = &guild.call
            && (call.info().needs_new_session(&info)
                || call.state() == crate::CallState::WaitingForServer)
        {
            call.update_server(info.clone());
        }
        for waiter in guild.waiters.drain(..) {
            let _ = waiter.send(info.clone());
        }
    }

    /// Joins a voice channel, or moves the call of the guild to another channel. Gives the
    /// call when the voice session is ready.
    ///
    /// # Errors
    ///
    /// Gives an error when the user ID is not known, the gateway command fails, the voice
    /// events do not arrive in time, or the voice connection fails.
    pub async fn join(&self, guild_id: GuildId, channel_id: ChannelId) -> Result<Call, JoinError> {
        if lock(&self.inner.user_id).is_none() {
            return Err(JoinError::UnknownUser);
        }
        let config = &self.inner.config;
        let (sender, receiver) = oneshot::channel();
        {
            let mut guilds = lock(&self.inner.guilds);
            let guild = guilds.entry(guild_id).or_default();
            if let Some(call) = &guild.call
                && guild.channel_id == Some(channel_id)
                && call.state() == crate::CallState::Ready
            {
                return Ok(call.clone());
            }
            // Wait for new events: the old token and session are not valid after a move.
            guild.session_id = None;
            guild.token = None;
            guild.endpoint = None;
            guild.waiters.push(sender);
        }

        let update = UpdateVoiceState::new(
            guild_id,
            Some(channel_id),
            config.self_mute,
            config.self_deaf,
        );
        self.inner
            .gateway
            .update_voice_state(update)
            .await
            .map_err(JoinError::Gateway)?;

        let info = tokio::time::timeout(config.timeout, receiver)
            .await
            .map_err(|_| JoinError::Timeout)?
            .map_err(|_| JoinError::Left)?;

        let call = {
            let mut guilds = lock(&self.inner.guilds);
            let guild = guilds.entry(guild_id).or_default();
            match &guild.call {
                Some(call) => call.clone(),
                None => {
                    let call = Call::connect(info, config.clone(), (self.inner.e2ee)());
                    guild.call = Some(call.clone());
                    call
                }
            }
        };
        call.wait_ready(config.timeout).await?;
        Ok(call)
    }

    /// Leaves the voice channel of a guild and stops its call.
    ///
    /// # Errors
    ///
    /// Gives an error when the gateway command fails. The call stops in all cases.
    pub async fn leave(&self, guild_id: GuildId) -> Result<(), JoinError> {
        let guild = lock(&self.inner.guilds).remove(&guild_id);
        if let Some(call) = guild.and_then(|guild| guild.call) {
            call.disconnect();
        }
        let update = UpdateVoiceState::new(guild_id, None, false, false);
        self.inner
            .gateway
            .update_voice_state(update)
            .await
            .map_err(JoinError::Gateway)
    }
}
