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
use std::sync::atomic::{AtomicU64, Ordering};
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
/// It is implemented for one [`ShardHandle`] and for [`ShardHandles`] (many shards, routed
/// by guild).
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

/// The shard handles of a process, with the total shard count of the bot.
///
/// The count is necessary because a process can run only some shards: the shard of a guild
/// comes from the total count, not from the number of handles.
#[derive(Clone, Debug)]
pub struct ShardHandles {
    shards: Vec<ShardHandle>,
    count: u32,
}

impl ShardHandles {
    /// Makes the list from the handles and the total shard count of the bot.
    #[must_use]
    pub fn new(shards: Vec<ShardHandle>, count: u32) -> Self {
        Self {
            shards,
            count: count.max(1),
        }
    }
}

impl VoiceGateway for ShardHandles {
    fn update_voice_state(&self, update: UpdateVoiceState) -> GatewayFuture<'_> {
        Box::pin(async move {
            let shard_id = shard_id_for_guild(update.guild_id, self.count);
            let shard = self
                .shards
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
    /// A number that is new each time the entry is made. A join checks it to find a leave.
    generation: u64,
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
    generations: AtomicU64,
    config: CallConfig,
    e2ee: Box<E2eeFactory>,
}

impl ManagerInner {
    /// Gives the entry of a guild. Makes it, with a new generation, when it is not there.
    fn guild<'a>(
        &self,
        guilds: &'a mut HashMap<GuildId, GuildVoice>,
        guild_id: GuildId,
    ) -> &'a mut GuildVoice {
        guilds.entry(guild_id).or_insert_with(|| GuildVoice {
            generation: self.generations.fetch_add(1, Ordering::Relaxed),
            ..GuildVoice::default()
        })
    }
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
/// use cordial_voice::{CallConfig, DaveSession, E2eeSession, OggOpusSource, VoiceManager};
///
/// let manager = VoiceManager::with_e2ee(shard, CallConfig::default(), || {
///     Box::new(DaveSession::new().expect("the DAVE key pair is made")) as Box<dyn E2eeSession>
/// });
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
    /// Makes a manager with the default call settings and DAVE end-to-end encryption.
    ///
    /// Discord needs DAVE in voice channels (close code 4017).
    #[must_use]
    pub fn new(gateway: impl VoiceGateway) -> Self {
        Self::with_e2ee(gateway, CallConfig::default(), dave_session)
    }

    /// Makes a manager with no end-to-end encryption. Discord closes such calls with code
    /// 4017, so use it only for tests with a local voice server.
    #[must_use]
    pub fn without_e2ee(gateway: impl VoiceGateway) -> Self {
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
                generations: AtomicU64::new(0),
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
                self.on_voice_server(
                    update.guild_id,
                    update.token.expose(),
                    update.endpoint.as_deref(),
                );
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

        // The call gets the new channel through `complete`: a new channel needs a new session.
        let guild = self.inner.guild(&mut guilds, guild_id);
        guild.session_id = Some(session_id.to_owned());
        guild.channel_id = Some(channel_id);
        self.complete(guild_id, guild);
    }

    fn on_voice_server(&self, guild_id: GuildId, token: &str, endpoint: Option<&str>) {
        let mut guilds = lock(&self.inner.guilds);
        let guild = self.inner.guild(&mut guilds, guild_id);
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

        if guild.call.as_ref().is_some_and(Call::is_stopped) {
            // A stopped call does not connect again. The next join makes a new call.
            guild.call = None;
        }
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
        let generation = {
            let mut guilds = lock(&self.inner.guilds);
            let guild = self.inner.guild(&mut guilds, guild_id);
            if guild.call.as_ref().is_some_and(Call::is_stopped) {
                guild.call = None;
            }
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
            guild.generation
        };

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

        let (call, created) = {
            let mut guilds = lock(&self.inner.guilds);
            // A leave (or a disconnect) while the join waited removed the entry. A new entry
            // has a new generation.
            let Some(guild) = guilds
                .get_mut(&guild_id)
                .filter(|guild| guild.generation == generation)
            else {
                return Err(JoinError::Left);
            };
            match &guild.call {
                Some(call) if !call.is_stopped() => (call.clone(), false),
                _ => {
                    let call = Call::connect(info, config.clone(), (self.inner.e2ee)());
                    guild.call = Some(call.clone());
                    (call, true)
                }
            }
        };

        if let Err(error) = call.wait_ready(config.timeout).await {
            // Without this, a call that cannot connect tries again for ever.
            if created {
                self.forget_call(guild_id, &call);
            }
            return Err(error.into());
        }
        Ok(call)
    }

    /// Stops a call and removes it from the manager, if the manager still has it.
    fn forget_call(&self, guild_id: GuildId, call: &Call) {
        if let Some(guild) = lock(&self.inner.guilds).get_mut(&guild_id)
            && guild
                .call
                .as_ref()
                .is_some_and(|current| current.same(call))
        {
            guild.call = None;
        }
        call.disconnect();
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

/// Makes a DAVE session for a new call.
fn dave_session() -> Box<dyn E2eeSession> {
    match crate::dave::DaveSession::new() {
        Ok(session) => Box::new(session),
        Err(error) => {
            // Only a failure of the crypto setup gets here. The call then fails with close
            // code 4017, which names the problem.
            tracing::error!(%error, "cannot make a DAVE session; the call has no E2EE");
            Box::new(NoE2ee::default())
        }
    }
}
