//! The sans-IO core of a shard: the gateway protocol without a socket and without a clock.
//!
//! The driver gives the core what happens (a text frame, a close, the time), and the core gives
//! back actions (send a frame, reconnect, dispatch an event). Tests drive the core with scripted
//! frames and synthetic time. See `docs/research/01-gateway.md`.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use cordial_model::gateway::{
    Capabilities, CloseCode, Hello, Identify, IdentifyProperties, Intents, OpCode, RawPayload,
    RequestChannelInfo, RequestGuildMembers, RequestSoundboardSounds, Resume, SendPayload,
    UpdatePresence, UpdateVoiceState,
};
use cordial_model::id::GuildId;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

/// The maximum size of a payload that a client sends.
pub const MAX_PAYLOAD_SIZE: usize = 4096;
/// The number of payloads that a connection can send in each window.
pub const SEND_LIMIT: u32 = 120;
/// The window of the send limit.
pub const SEND_WINDOW: Duration = Duration::from_secs(60);
/// The number of sends in each window that only heartbeats, Identify, and Resume can use.
pub const RESERVED_SENDS: u32 = 3;
/// The time between two all-member requests for one guild.
pub const MEMBER_REQUEST_INTERVAL: Duration = Duration::from_secs(30);
/// The close code that the core uses to reconnect and keep the session (not 1000 or 1001).
pub const RESUME_CLOSE_CODE: u16 = 4900;
/// The time that a new connection waits for Hello before it connects again.
const HELLO_TIMEOUT: Duration = Duration::from_secs(20);

/// The settings of a shard core.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct CoreConfig {
    /// The bot token, without the `Bot ` prefix.
    pub token: String,
    /// The intents.
    pub intents: Intents,
    /// The shard ID and the shard count.
    pub shard: [u32; 2],
    /// The member count above which Discord does not send offline members (50 to 250).
    pub large_threshold: Option<u8>,
    /// The presence to send with Identify.
    pub presence: Option<UpdatePresence>,
    /// The connection properties.
    pub properties: IdentifyProperties,
    /// The capabilities.
    pub capabilities: Capabilities,
}

impl CoreConfig {
    /// Makes settings with a token and intents, for shard 0 of 1.
    #[must_use]
    pub fn new(token: impl Into<String>, intents: Intents) -> Self {
        Self {
            token: token.into(),
            intents,
            shard: [0, 1],
            large_threshold: None,
            presence: None,
            properties: IdentifyProperties::new("cordial"),
            capabilities: Capabilities::empty(),
        }
    }
}

/// The data that a session needs to resume.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Session {
    /// The session ID.
    pub id: String,
    /// The last sequence number.
    pub sequence: u64,
    /// The URL to resume the session.
    pub resume_url: String,
}

impl Session {
    /// Makes session data, for example to resume after a process restart.
    #[must_use]
    pub fn new(id: String, sequence: u64, resume_url: String) -> Self {
        Self {
            id,
            sequence,
            resume_url,
        }
    }
}

/// A command that the bot sends over the gateway.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Command {
    /// Update the presence (opcode 3).
    UpdatePresence(UpdatePresence),
    /// Join, move, or leave a voice channel (opcode 4).
    UpdateVoiceState(UpdateVoiceState),
    /// Request guild members (opcode 8).
    RequestGuildMembers(RequestGuildMembers),
    /// Request soundboard sounds (opcode 31).
    RequestSoundboardSounds(RequestSoundboardSounds),
    /// Request channel info (opcode 43).
    RequestChannelInfo(RequestChannelInfo),
}

/// The reason a command was not accepted.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CommandError {
    /// The payload is larger than 4096 bytes.
    #[error("the payload has {size} bytes; the maximum is {MAX_PAYLOAD_SIZE}")]
    TooLarge {
        /// The size in bytes.
        size: usize,
    },
    /// Discord permits one all-member request for each guild every 30 seconds.
    #[error("an all-member request for this guild must wait {retry_after:?}")]
    RateLimited {
        /// The time to wait.
        retry_after: Duration,
    },
    /// The payload could not be encoded.
    #[error("cannot encode the command: {0}")]
    Encode(String),
}

/// How the driver connects again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReconnectKind {
    /// Resume the session at its resume URL.
    Resume,
    /// Start a new session at the gateway URL.
    Identify,
}

/// A lifecycle change of the session.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Lifecycle {
    /// The gateway sent Hello.
    Hello {
        /// The heartbeat interval.
        heartbeat_interval: Duration,
    },
    /// A new session is ready.
    Ready {
        /// The session ID.
        session_id: String,
    },
    /// The session resumed.
    Resumed,
    /// The gateway invalidated the session.
    InvalidSession {
        /// Tells if the session can resume.
        resumable: bool,
    },
    /// The gateway asked for a reconnect.
    ReconnectRequested,
    /// No heartbeat acknowledgement arrived. The connection is a zombie.
    Zombie,
    /// A heartbeat acknowledgement arrived.
    HeartbeatAck {
        /// The time between the heartbeat and the acknowledgement.
        latency: Duration,
    },
}

/// A fatal error: the shard must stop.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum FatalError {
    /// The gateway closed with a code that does not permit a reconnect.
    #[error("the gateway closed the connection with code {code}: {}", describe_close(*code))]
    Closed {
        /// The close code.
        code: u16,
    },
}

fn describe_close(code: u16) -> &'static str {
    match CloseCode::from_value(code) {
        CloseCode::AuthenticationFailed => "the token is not valid",
        CloseCode::InvalidShard => "the shard ID or the shard count is not valid",
        CloseCode::ShardingRequired => "the bot is in too many guilds and needs more shards",
        CloseCode::InvalidApiVersion => "the API version is not valid",
        CloseCode::InvalidIntents => "the intents are not valid",
        CloseCode::DisallowedIntents => {
            "a privileged intent is not turned on in the Developer Portal or not approved"
        }
        _ => "see the gateway close codes",
    }
}

/// An action that the driver must do.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Action {
    /// Send a text frame.
    Send(String),
    /// Wait for a permit of the identify queue, then call [`ShardCore::identify`].
    RequestIdentify,
    /// Close the connection (if it is open) and connect again after the delay.
    Reconnect {
        /// Resume or identify.
        kind: ReconnectKind,
        /// The close code to use, if the connection is open.
        close_code: u16,
        /// The time to wait before the new connection.
        delay: Duration,
    },
    /// Give an event to the consumer.
    Dispatch {
        /// The event name.
        name: String,
        /// The raw event data.
        data: Box<RawValue>,
    },
    /// Report a lifecycle change.
    Lifecycle(Lifecycle),
    /// Stop the shard.
    Fatal(FatalError),
}

impl PartialEq for Action {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Send(first), Self::Send(second)) => first == second,
            (Self::RequestIdentify, Self::RequestIdentify) => true,
            (
                Self::Reconnect {
                    kind,
                    close_code,
                    delay,
                },
                Self::Reconnect {
                    kind: other_kind,
                    close_code: other_code,
                    delay: other_delay,
                },
            ) => kind == other_kind && close_code == other_code && delay == other_delay,
            (
                Self::Dispatch { name, data },
                Self::Dispatch {
                    name: other_name,
                    data: other_data,
                },
            ) => name == other_name && data.get() == other_data.get(),
            (Self::Lifecycle(first), Self::Lifecycle(second)) => first == second,
            (Self::Fatal(first), Self::Fatal(second)) => first == second,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Disconnected,
    AwaitingHello,
    AwaitingIdentifyPermit,
    Identifying,
    Resuming,
    Active,
    Stopped,
}

/// The sans-IO state machine of one shard.
#[derive(Debug)]
pub struct ShardCore {
    config: CoreConfig,
    state: State,
    session: Option<Session>,
    sequence: Option<u64>,
    heartbeat_interval: Option<Duration>,
    hello_deadline: Option<Instant>,
    next_heartbeat: Option<Instant>,
    heartbeat_sent_at: Option<Instant>,
    ack_pending: bool,
    latency: Option<Duration>,
    limiter: SendLimiter,
    pending: VecDeque<String>,
    member_requests: HashMap<GuildId, Instant>,
    actions: VecDeque<Queued>,
    random: u64,
}

/// An action in the queue. A command send goes back to `pending` when the connection ends
/// before the driver takes it.
#[derive(Debug)]
struct Queued {
    action: Action,
    command: bool,
}

impl ShardCore {
    /// Makes a core.
    #[must_use]
    pub fn new(config: CoreConfig, now: Instant) -> Self {
        let seed = u64::from(config.shard[0]).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |time| time.as_secs() ^ u64::from(time.subsec_nanos()));
        Self {
            config,
            state: State::Disconnected,
            session: None,
            sequence: None,
            heartbeat_interval: None,
            hello_deadline: None,
            next_heartbeat: None,
            heartbeat_sent_at: None,
            ack_pending: false,
            latency: None,
            limiter: SendLimiter::new(now),
            pending: VecDeque::new(),
            member_requests: HashMap::new(),
            actions: VecDeque::new(),
            random: seed | 1,
        }
    }

    /// Makes a core that resumes a saved session on the first connection.
    #[must_use]
    pub fn with_session(config: CoreConfig, session: Session, now: Instant) -> Self {
        let mut core = Self::new(config, now);
        core.sequence = Some(session.sequence);
        core.session = Some(session);
        core
    }

    /// Gives the session, for example to save it before a process restart.
    #[must_use]
    pub fn session(&self) -> Option<Session> {
        let mut session = self.session.clone()?;
        session.sequence = self.sequence.unwrap_or(session.sequence);
        Some(session)
    }

    /// Gives the shard ID and the shard count.
    #[must_use]
    pub fn shard(&self) -> [u32; 2] {
        self.config.shard
    }

    /// Gives the last heartbeat latency.
    #[must_use]
    pub fn latency(&self) -> Option<Duration> {
        self.latency
    }

    /// Tells if the session is ready (or resumed) and receives events.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state == State::Active
    }

    /// Tells if the shard stopped because of a fatal error.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.state == State::Stopped
    }

    /// Tells if the next connection resumes a session.
    #[must_use]
    pub fn will_resume(&self) -> bool {
        self.session.is_some()
    }

    /// Gives the URL for the next connection: the resume URL or `None` for the gateway URL.
    #[must_use]
    pub fn resume_url(&self) -> Option<&str> {
        self.session
            .as_ref()
            .map(|session| session.resume_url.as_str())
    }

    /// Takes the next action.
    pub fn poll_action(&mut self) -> Option<Action> {
        self.actions.pop_front().map(|queued| queued.action)
    }

    /// Gives the next time at which [`ShardCore::tick`] must run.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        // Only an active session flushes the queue. In other states, the end of the send
        // window is not a reason to wake up, and an old window end makes the driver spin.
        let flush = (self.state == State::Active && !self.pending.is_empty())
            .then(|| self.limiter.window_end());
        [self.next_heartbeat, self.hello_deadline, flush]
            .into_iter()
            .flatten()
            .min()
    }

    /// Reports that a new WebSocket connection is open.
    ///
    /// When Hello does not arrive within 20 seconds, the core connects again.
    pub fn connected(&mut self, now: Instant) {
        self.take_back_unsent();
        self.state = State::AwaitingHello;
        self.heartbeat_interval = None;
        self.hello_deadline = Some(now + HELLO_TIMEOUT);
        self.next_heartbeat = None;
        self.heartbeat_sent_at = None;
        self.ack_pending = false;
        self.limiter = SendLimiter::new(now);
    }

    /// Reports that the connection closed, with the close code if one arrived.
    ///
    /// Also call it when a send fails. Then the core sends nothing more on the old connection.
    pub fn closed(&mut self, code: Option<u16>, _now: Instant) {
        self.take_back_unsent();
        if self.state == State::Stopped {
            return;
        }
        // A reconnect is already in the queue. A second one would close the next connection.
        let reconnect_queued = self.state == State::Disconnected;
        self.state = State::Disconnected;
        self.hello_deadline = None;
        self.next_heartbeat = None;

        let Some(code) = code else {
            if !reconnect_queued {
                self.reconnect(ReconnectKind::Resume, Duration::ZERO);
            }
            return;
        };

        let close = CloseCode::from_value(code);
        if !close.is_unknown() && !close.can_reconnect() {
            self.state = State::Stopped;
            self.push_action(Action::Fatal(FatalError::Closed { code }));
            return;
        }

        // The next connection resumes only when a session is left, so a queued reconnect
        // stays correct after the session is cleared.
        let kind = if close.needs_new_session() {
            self.session = None;
            self.sequence = None;
            ReconnectKind::Identify
        } else {
            ReconnectKind::Resume
        };
        if !reconnect_queued {
            self.reconnect(kind, Duration::ZERO);
        }
    }

    /// Reports that the identify queue gave a permit. Sends Identify.
    pub fn identify(&mut self, now: Instant) {
        if self.state != State::AwaitingIdentifyPermit {
            return;
        }

        let identify = Identify::new(
            self.config.token.clone(),
            self.config.properties.clone(),
            self.config.intents,
            self.config.shard,
        )
        .large_threshold(self.config.large_threshold)
        .presence(self.config.presence.clone())
        .capabilities(self.config.capabilities);
        let payload = encode(OpCode::Identify, &identify).unwrap_or_default();
        self.state = State::Identifying;
        self.send_essential(payload, now);
    }

    /// Queues a command. It goes out when the send limit permits.
    ///
    /// # Errors
    ///
    /// Gives an error when the payload is too large or an all-member request must wait.
    pub fn send_command(&mut self, command: &Command, now: Instant) -> Result<(), CommandError> {
        let payload = match command {
            Command::UpdatePresence(presence) => {
                self.config.presence = Some(presence.clone());
                encode(OpCode::PresenceUpdate, presence)
            }
            Command::UpdateVoiceState(state) => encode(OpCode::VoiceStateUpdate, state),
            Command::RequestGuildMembers(request) => {
                if request.is_all_members() {
                    self.check_member_request(request.guild_id, now)?;
                }
                encode(OpCode::RequestGuildMembers, request)
            }
            Command::RequestSoundboardSounds(request) => {
                encode(OpCode::RequestSoundboardSounds, request)
            }
            Command::RequestChannelInfo(request) => encode(OpCode::RequestChannelInfo, request),
        }
        .map_err(|error| CommandError::Encode(error.to_string()))?;

        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(CommandError::TooLarge {
                size: payload.len(),
            });
        }

        self.pending.push_back(payload);
        self.flush(now);
        Ok(())
    }

    fn check_member_request(
        &mut self,
        guild_id: GuildId,
        now: Instant,
    ) -> Result<(), CommandError> {
        if let Some(last) = self.member_requests.get(&guild_id) {
            let elapsed = now.saturating_duration_since(*last);
            if elapsed < MEMBER_REQUEST_INTERVAL {
                return Err(CommandError::RateLimited {
                    retry_after: MEMBER_REQUEST_INTERVAL - elapsed,
                });
            }
        }
        self.member_requests.insert(guild_id, now);
        Ok(())
    }

    /// Reports that the time is `now`. Sends due heartbeats and queued commands.
    pub fn tick(&mut self, now: Instant) {
        if let Some(due) = self.hello_deadline
            && now >= due
        {
            tracing::warn!(shard = self.config.shard[0], "no Hello; reconnecting");
            self.state = State::Disconnected;
            self.hello_deadline = None;
            self.reconnect(ReconnectKind::Resume, Duration::ZERO);
            return;
        }

        if let Some(due) = self.next_heartbeat
            && now >= due
        {
            if self.ack_pending {
                // No acknowledgement since the last heartbeat: the connection is dead.
                tracing::warn!(
                    shard = self.config.shard[0],
                    "no heartbeat acknowledgement; reconnecting"
                );
                self.push_action(Action::Lifecycle(Lifecycle::Zombie));
                self.state = State::Disconnected;
                self.next_heartbeat = None;
                self.reconnect(ReconnectKind::Resume, Duration::ZERO);
                return;
            }
            self.send_heartbeat(now);
        }

        self.flush(now);
    }

    /// Reports a text frame (after decompression).
    ///
    /// # Errors
    ///
    /// Gives an error when the frame is not a valid gateway payload.
    pub fn received(&mut self, text: &str, now: Instant) -> Result<(), serde_json::Error> {
        let payload: RawPayload<'_> = serde_json::from_str(text)?;

        match payload.op {
            OpCode::Dispatch => self.on_dispatch(&payload, now)?,
            OpCode::Heartbeat => self.send_heartbeat(now),
            OpCode::HeartbeatAck => self.on_heartbeat_ack(now),
            OpCode::Hello => {
                let data = payload.d.map_or("null", RawValue::get);
                let hello: Hello = serde_json::from_str(data)?;
                self.on_hello(&hello, now);
            }
            OpCode::Reconnect => {
                self.push_action(Action::Lifecycle(Lifecycle::ReconnectRequested));
                self.state = State::Disconnected;
                self.hello_deadline = None;
                self.next_heartbeat = None;
                self.reconnect(ReconnectKind::Resume, Duration::ZERO);
            }
            OpCode::InvalidSession => {
                let resumable = payload.d.is_some_and(|data| data.get().trim() == "true");
                self.on_invalid_session(resumable);
            }
            _ => {
                tracing::debug!(op = payload.op.value(), "unknown or unexpected opcode");
            }
        }
        Ok(())
    }

    fn on_hello(&mut self, hello: &Hello, now: Instant) {
        let interval = Duration::from_millis(hello.heartbeat_interval);
        self.hello_deadline = None;
        self.heartbeat_interval = Some(interval);
        self.next_heartbeat = Some(now + interval.mul_f64(self.next_random_fraction()));
        self.ack_pending = false;
        self.push_action(Action::Lifecycle(Lifecycle::Hello {
            heartbeat_interval: interval,
        }));

        match (&self.session, self.sequence) {
            (Some(session), Some(sequence)) => {
                let resume = Resume::new(self.config.token.clone(), session.id.clone(), sequence);
                let payload = encode(OpCode::Resume, &resume).unwrap_or_default();
                self.state = State::Resuming;
                self.send_essential(payload, now);
            }
            _ => {
                self.state = State::AwaitingIdentifyPermit;
                self.push_action(Action::RequestIdentify);
            }
        }
    }

    fn on_heartbeat_ack(&mut self, now: Instant) {
        self.ack_pending = false;
        if let Some(sent_at) = self.heartbeat_sent_at {
            let latency = now.saturating_duration_since(sent_at);
            self.latency = Some(latency);
            self.push_action(Action::Lifecycle(Lifecycle::HeartbeatAck { latency }));
        }
    }

    fn on_invalid_session(&mut self, resumable: bool) {
        self.push_action(Action::Lifecycle(Lifecycle::InvalidSession { resumable }));
        self.state = State::Disconnected;
        self.hello_deadline = None;
        self.next_heartbeat = None;

        if resumable && self.session.is_some() {
            self.reconnect(ReconnectKind::Resume, Duration::ZERO);
        } else {
            self.session = None;
            self.sequence = None;
            // The docs no longer give a wait time. A random 1 to 5 seconds avoids a burst.
            let delay = Duration::from_secs(1)
                + Duration::from_secs(4).mul_f64(self.next_random_fraction());
            self.reconnect(ReconnectKind::Identify, delay);
        }
    }

    fn on_dispatch(
        &mut self,
        payload: &RawPayload<'_>,
        now: Instant,
    ) -> Result<(), serde_json::Error> {
        if let Some(sequence) = payload.s {
            self.sequence = Some(
                self.sequence
                    .map_or(sequence, |current| current.max(sequence)),
            );
        }
        let Some(name) = payload.t else {
            return Ok(());
        };
        let data = payload.d.map_or_else(
            || RawValue::from_string("null".to_owned()),
            |data| Ok(data.to_owned()),
        )?;

        match name {
            "READY" => {
                #[derive(Deserialize)]
                struct ReadySession {
                    session_id: String,
                    resume_gateway_url: String,
                }
                let ready: ReadySession = serde_json::from_str(data.get())?;
                self.session = Some(Session {
                    id: ready.session_id.clone(),
                    sequence: self.sequence.unwrap_or(0),
                    resume_url: ready.resume_gateway_url,
                });
                self.state = State::Active;
                self.push_action(Action::Lifecycle(Lifecycle::Ready {
                    session_id: ready.session_id,
                }));
                self.flush(now);
            }
            "RESUMED" => {
                self.state = State::Active;
                self.push_action(Action::Lifecycle(Lifecycle::Resumed));
                self.flush(now);
            }
            _ => {}
        }

        self.push_action(Action::Dispatch {
            name: name.to_owned(),
            data,
        });
        Ok(())
    }

    fn send_heartbeat(&mut self, now: Instant) {
        let payload = match self.sequence {
            Some(sequence) => format!(r#"{{"op":1,"d":{sequence}}}"#),
            None => r#"{"op":1,"d":null}"#.to_owned(),
        };
        self.send_essential(payload, now);
        self.heartbeat_sent_at = Some(now);
        self.ack_pending = true;
        if let Some(interval) = self.heartbeat_interval {
            self.next_heartbeat = Some(now + interval);
        }
    }

    fn send_essential(&mut self, payload: String, now: Instant) {
        // Essential payloads use the reserved part of the send limit. They go out even when
        // the limit is reached, because a missed heartbeat ends the session.
        if !self.limiter.try_take(now, true) {
            tracing::warn!("send limit reached; an essential payload goes out anyway");
        }
        self.push_action(Action::Send(payload));
    }

    fn flush(&mut self, now: Instant) {
        if self.state != State::Active {
            return;
        }
        while !self.pending.is_empty() && self.limiter.try_take(now, false) {
            if let Some(payload) = self.pending.pop_front() {
                self.actions.push_back(Queued {
                    action: Action::Send(payload),
                    command: true,
                });
            }
        }
    }

    /// Removes the sends of the old connection from the queue. Commands go back to the front
    /// of `pending` in their order, so that the next session sends them.
    fn take_back_unsent(&mut self) {
        let mut unsent = Vec::new();
        self.actions.retain_mut(|queued| match &mut queued.action {
            Action::Send(payload) => {
                if queued.command {
                    unsent.push(std::mem::take(payload));
                }
                false
            }
            _ => true,
        });
        for payload in unsent.into_iter().rev() {
            self.pending.push_front(payload);
        }
    }

    fn push_action(&mut self, action: Action) {
        self.actions.push_back(Queued {
            action,
            command: false,
        });
    }

    fn reconnect(&mut self, kind: ReconnectKind, delay: Duration) {
        let kind = if kind == ReconnectKind::Resume && self.session.is_none() {
            ReconnectKind::Identify
        } else {
            kind
        };
        self.push_action(Action::Reconnect {
            kind,
            close_code: RESUME_CLOSE_CODE,
            delay,
        });
    }

    /// Gives a number in [0, 1) from a small xorshift generator. It needs no security.
    fn next_random_fraction(&mut self) -> f64 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        #[allow(clippy::cast_precision_loss)]
        let fraction = (self.random >> 11) as f64 / (1_u64 << 53) as f64;
        fraction
    }
}

fn encode<T: Serialize>(op: OpCode, data: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string(&SendPayload::new(op, data))
}

/// The send limit of one connection: 120 payloads in each 60 seconds.
#[derive(Debug)]
struct SendLimiter {
    window_start: Instant,
    count: u32,
}

impl SendLimiter {
    fn new(now: Instant) -> Self {
        Self {
            window_start: now,
            count: 0,
        }
    }

    fn try_take(&mut self, now: Instant, essential: bool) -> bool {
        if now.saturating_duration_since(self.window_start) >= SEND_WINDOW {
            self.window_start = now;
            self.count = 0;
        }
        let capacity = if essential {
            SEND_LIMIT
        } else {
            SEND_LIMIT - RESERVED_SENDS
        };
        if self.count < capacity {
            self.count += 1;
            true
        } else {
            false
        }
    }

    fn window_end(&self) -> Instant {
        self.window_start + SEND_WINDOW
    }
}
