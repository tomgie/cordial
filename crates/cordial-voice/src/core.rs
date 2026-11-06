//! The sans-IO core of a voice connection: the voice gateway protocol without a socket and
//! without a clock.
//!
//! The driver gives the core what happens (a text or binary frame, a close, the result of IP
//! discovery, the time). The core gives back actions (send a frame, open UDP, start the
//! media, reconnect). Tests drive the core with scripted frames and synthetic time.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use cordial_model::id::{ChannelId, GuildId, UserId};
use serde::de::DeserializeOwned;

use crate::crypto::SecretKey;
use crate::e2ee::{E2eeOutgoing, E2eeSession};
use crate::protocol::{
    ClientDisconnect, ClientSsrc, ClientsConnect, CloseAction, EncryptionMode, Heartbeat,
    HeartbeatAck, Hello, Identify, Ready, ReceivedPayload, Resume, SelectProtocol,
    SessionDescription, Speaking, SpeakingFlags, VoiceCloseCode, VoiceOpCode, encode,
};

/// The number of new sessions in a row that can fail before the core stops.
pub const MAX_SESSION_FAILURES: u32 = 3;
/// The number of resumes in a row that can fail before the core starts a new session.
const MAX_RESUME_ATTEMPTS: u32 = 5;
/// The longest delay before a resume.
const MAX_RESUME_DELAY: Duration = Duration::from_secs(30);

/// An E2EE session that the core and the media tasks share.
pub type SharedE2ee = Arc<Mutex<Box<dyn E2eeSession>>>;

/// Locks a shared E2EE session. A poisoned lock still gives the session.
pub(crate) fn lock_e2ee(e2ee: &SharedE2ee) -> MutexGuard<'_, Box<dyn E2eeSession>> {
    e2ee.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The data that a voice connection needs: from `VOICE_STATE_UPDATE` and
/// `VOICE_SERVER_UPDATE`.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConnectionInfo {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The voice channel ID.
    pub channel_id: ChannelId,
    /// The user ID of the bot.
    pub user_id: UserId,
    /// The session ID from `VOICE_STATE_UPDATE`.
    pub session_id: String,
    /// The token from `VOICE_SERVER_UPDATE`.
    pub token: String,
    /// The endpoint from `VOICE_SERVER_UPDATE`, for example `sweetwater-12345.discord.media:2048`.
    ///
    /// An endpoint that starts with `ws://` or `wss://` is used as the URL base. Tests use it
    /// for a local server.
    pub endpoint: String,
}

impl ConnectionInfo {
    /// Makes connection data.
    #[must_use]
    pub fn new(
        guild_id: GuildId,
        channel_id: ChannelId,
        user_id: UserId,
        session_id: String,
        token: String,
        endpoint: String,
    ) -> Self {
        Self {
            guild_id,
            channel_id,
            user_id,
            session_id,
            token,
            endpoint,
        }
    }

    /// Gives the WebSocket URL of the voice gateway: `wss://{endpoint}/?v=8`.
    #[must_use]
    pub fn websocket_url(&self) -> String {
        let endpoint = self.endpoint.trim_end_matches('/');
        let version = crate::protocol::VOICE_GATEWAY_VERSION;
        if endpoint.starts_with("ws://") || endpoint.starts_with("wss://") {
            format!("{endpoint}/?v={version}")
        } else {
            format!("wss://{endpoint}/?v={version}")
        }
    }

    /// Tells if `other` needs a new voice session: the session, the token, the endpoint, or
    /// the channel changed.
    ///
    /// The channel is part of the compare because DAVE uses the channel ID as the group ID
    /// of the session.
    #[must_use]
    pub fn needs_new_session(&self, other: &Self) -> bool {
        self.session_id != other.session_id
            || self.channel_id != other.channel_id
            || self.token != other.token
            || self.endpoint != other.endpoint
            || self.guild_id != other.guild_id
            || self.user_id != other.user_id
    }
}

impl fmt::Debug for ConnectionInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConnectionInfo")
            .field("guild_id", &self.guild_id)
            .field("channel_id", &self.channel_id)
            .field("user_id", &self.user_id)
            .field("session_id", &self.session_id)
            .field("endpoint", &self.endpoint)
            .finish_non_exhaustive()
    }
}

/// How the driver connects again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReconnectKind {
    /// Resume the session. The UDP media continues.
    Resume,
    /// Identify with a new session. The driver stops the UDP media.
    NewSession,
}

/// A change that the core reports.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum CoreEvent {
    /// The server sent Hello.
    Hello {
        /// The heartbeat interval.
        heartbeat_interval: Duration,
    },
    /// The server sent Ready.
    Ready {
        /// The SSRC of the client.
        ssrc: u32,
    },
    /// The session resumed.
    Resumed,
    /// A heartbeat acknowledgement arrived.
    HeartbeatAck {
        /// The time between the heartbeat and the acknowledgement.
        latency: Duration,
    },
    /// No heartbeat acknowledgement arrived. The connection is dead.
    Zombie,
    /// A user uses an SSRC (opcode 5 or 12).
    UserSsrc {
        /// The user.
        user_id: UserId,
        /// The audio SSRC.
        ssrc: u32,
    },
    /// The speaking state of a user changed (opcode 5).
    Speaking {
        /// The user.
        user_id: UserId,
        /// The SSRC.
        ssrc: u32,
        /// The speaking flags.
        flags: SpeakingFlags,
    },
    /// Users joined the call (opcode 11).
    ClientsConnect(Vec<UserId>),
    /// A user left the call (opcode 13).
    ClientDisconnect(UserId),
    /// The E2EE session reported an error.
    E2eeError(String),
}

/// A fatal error: the connection stops.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum VoiceFatal {
    /// The voice gateway closed with a code that does not permit a reconnect.
    #[error("the voice gateway closed the connection: {0}")]
    Closed(VoiceCloseCode),
    /// The server offers no encryption mode that this crate supports.
    #[error("the voice server offers no supported encryption mode")]
    NoEncryptionMode,
    /// The Session Description is not valid.
    #[error("the session description is not valid: {0}")]
    BadSessionDescription(String),
    /// Too many new sessions failed in a row.
    #[error("{MAX_SESSION_FAILURES} new voice sessions failed in a row")]
    TooManyFailures,
}

/// An action that the driver must do.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum VoiceAction {
    /// Send a text frame.
    SendText(String),
    /// Send a binary frame.
    SendBinary(Vec<u8>),
    /// Open UDP to the voice server, do IP discovery, then call
    /// [`VoiceCore::ip_discovered`].
    ConnectUdp {
        /// The IP address of the voice server.
        ip: String,
        /// The UDP port of the voice server.
        port: u16,
        /// The SSRC of the client.
        ssrc: u32,
    },
    /// The session is ready: start to send and receive media.
    SessionReady {
        /// The transport encryption mode.
        mode: EncryptionMode,
        /// The key of the transport encryption.
        secret_key: SecretKey,
        /// The SSRC of the client.
        ssrc: u32,
        /// The DAVE protocol version.
        dave_protocol_version: u16,
    },
    /// Report a change.
    Event(CoreEvent),
    /// Close the connection (if it is open) and connect again after the delay.
    Reconnect {
        /// Resume or identify.
        kind: ReconnectKind,
        /// The time to wait before the new connection.
        delay: Duration,
    },
    /// Stop the connection.
    Fatal(VoiceFatal),
}

/// An error in a frame from the voice gateway.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FrameError {
    /// A JSON frame is not valid.
    #[error("the voice gateway sent JSON that is not valid: {0}")]
    Json(#[from] serde_json::Error),
    /// A binary frame is too short.
    #[error("the voice gateway sent a binary frame of {0} bytes")]
    ShortBinary(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Disconnected,
    AwaitingHello,
    Identifying,
    AwaitingDiscovery,
    SelectingProtocol,
    Resuming,
    Active,
    Stopped,
}

/// The sans-IO state machine of one voice connection.
pub struct VoiceCore {
    info: ConnectionInfo,
    e2ee: SharedE2ee,
    state: State,
    identified: bool,
    seq_ack: Option<i64>,
    heartbeat_interval: Option<Duration>,
    next_heartbeat: Option<Instant>,
    heartbeat_sent_at: Option<Instant>,
    ack_pending: bool,
    nonce: u64,
    latency: Option<Duration>,
    ssrc: Option<u32>,
    mode: Option<EncryptionMode>,
    speaking: SpeakingFlags,
    session_failures: u32,
    resume_attempts: u32,
    actions: VecDeque<VoiceAction>,
}

impl fmt::Debug for VoiceCore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VoiceCore")
            .field("info", &self.info)
            .field("state", &self.state)
            .field("seq_ack", &self.seq_ack)
            .field("ssrc", &self.ssrc)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl VoiceCore {
    /// Makes a core.
    #[must_use]
    pub fn new(info: ConnectionInfo, e2ee: SharedE2ee) -> Self {
        Self {
            info,
            e2ee,
            state: State::Disconnected,
            identified: false,
            seq_ack: None,
            heartbeat_interval: None,
            next_heartbeat: None,
            heartbeat_sent_at: None,
            ack_pending: false,
            nonce: 0,
            latency: None,
            ssrc: None,
            mode: None,
            speaking: SpeakingFlags::NONE,
            session_failures: 0,
            resume_attempts: 0,
            actions: VecDeque::new(),
        }
    }

    /// Gives the connection data.
    #[must_use]
    pub fn info(&self) -> &ConnectionInfo {
        &self.info
    }

    /// Gives the shared E2EE session.
    #[must_use]
    pub fn e2ee(&self) -> &SharedE2ee {
        &self.e2ee
    }

    /// Gives the SSRC of the client, after Ready.
    #[must_use]
    pub fn ssrc(&self) -> Option<u32> {
        self.ssrc
    }

    /// Gives the selected encryption mode, after Ready.
    #[must_use]
    pub fn mode(&self) -> Option<EncryptionMode> {
        self.mode
    }

    /// Gives the last heartbeat latency.
    #[must_use]
    pub fn latency(&self) -> Option<Duration> {
        self.latency
    }

    /// Gives the `seq_ack` value: the last sequence number received, or -1.
    #[must_use]
    pub fn seq_ack(&self) -> i64 {
        self.seq_ack.unwrap_or(-1)
    }

    /// Tells if the session is ready (or resumed) and media can flow.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.state == State::Active
    }

    /// Tells if the connection stopped because of a fatal error.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.state == State::Stopped
    }

    /// Tells if the next connection resumes the session.
    #[must_use]
    pub fn will_resume(&self) -> bool {
        self.identified
    }

    /// Takes the next action.
    pub fn poll_action(&mut self) -> Option<VoiceAction> {
        self.actions.pop_front()
    }

    /// Gives the next time at which [`VoiceCore::tick`] must run.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.next_heartbeat
    }

    /// Replaces the connection data, for example after a channel move. The next connection
    /// identifies with a new session.
    pub fn set_info(&mut self, info: ConnectionInfo) {
        self.info = info;
        self.forget_session();
        self.session_failures = 0;
        if self.state == State::Stopped {
            self.state = State::Disconnected;
        }
    }

    /// Reports that a new WebSocket connection is open.
    pub fn connected(&mut self, _now: Instant) {
        if self.state == State::Stopped {
            return;
        }
        self.state = State::AwaitingHello;
        self.heartbeat_interval = None;
        self.next_heartbeat = None;
        self.heartbeat_sent_at = None;
        self.ack_pending = false;
    }

    /// Reports that the connection closed, with the close code if one arrived.
    pub fn closed(&mut self, code: Option<u16>, _now: Instant) {
        if self.state == State::Stopped {
            return;
        }
        self.state = State::Disconnected;
        self.next_heartbeat = None;

        let action = code.map_or(CloseAction::Resume, |code| {
            VoiceCloseCode::from_value(code).action()
        });
        match action {
            CloseAction::Resume => self.reconnect(ReconnectKind::Resume),
            CloseAction::NewSession => {
                self.forget_session();
                self.reconnect(ReconnectKind::NewSession);
            }
            CloseAction::Stop => {
                let code = VoiceCloseCode::from_value(code.unwrap_or_default());
                self.stop(VoiceFatal::Closed(code));
            }
        }
    }

    /// Reports that the time is `now`. Sends a due heartbeat.
    pub fn tick(&mut self, now: Instant) {
        let Some(due) = self.next_heartbeat else {
            return;
        };
        if now < due {
            return;
        }

        if self.ack_pending {
            tracing::warn!("no voice heartbeat acknowledgement; reconnecting");
            self.push_event(CoreEvent::Zombie);
            self.state = State::Disconnected;
            self.next_heartbeat = None;
            self.reconnect(ReconnectKind::Resume);
            return;
        }
        self.send_heartbeat(now);
    }

    /// Reports the external address from IP discovery. Sends Select Protocol.
    pub fn ip_discovered(&mut self, address: String, port: u16, _now: Instant) {
        if self.state != State::AwaitingDiscovery {
            return;
        }
        let Some(mode) = self.mode else {
            return;
        };
        let select = SelectProtocol::udp(address, port, mode);
        self.send_text(encode(VoiceOpCode::SelectProtocol, &select));
        self.state = State::SelectingProtocol;
    }

    /// Reports that IP discovery failed. The core starts a new session.
    pub fn ip_discovery_failed(&mut self, _now: Instant) {
        if self.state != State::AwaitingDiscovery {
            return;
        }
        self.state = State::Disconnected;
        self.next_heartbeat = None;
        self.forget_session();
        self.reconnect(ReconnectKind::NewSession);
    }

    /// Sends the speaking flags (opcode 5). Gives `false` when the session is not active.
    pub fn set_speaking(&mut self, flags: SpeakingFlags) -> bool {
        self.speaking = flags;
        let Some(ssrc) = self.ssrc else {
            return false;
        };
        if self.state != State::Active {
            return false;
        }
        self.send_text(encode(VoiceOpCode::Speaking, &Speaking::new(flags, ssrc)));
        true
    }

    /// Gives the last speaking flags.
    #[must_use]
    pub fn speaking(&self) -> SpeakingFlags {
        self.speaking
    }

    /// Reports a text frame.
    ///
    /// # Errors
    ///
    /// Gives an error when the frame is not a valid voice gateway payload.
    pub fn received_text(&mut self, text: &str, now: Instant) -> Result<(), FrameError> {
        let payload: ReceivedPayload = serde_json::from_str(text)?;
        if let Some(seq) = payload.seq {
            self.seq_ack = Some(seq);
        }

        let Some(op) = VoiceOpCode::from_value(payload.op) else {
            tracing::debug!(op = payload.op, "unknown voice opcode");
            return Ok(());
        };
        match op {
            VoiceOpCode::Hello => {
                let hello: Hello = decode(payload.d)?;
                self.on_hello(&hello, now);
            }
            VoiceOpCode::Ready => {
                let ready: Ready = decode(payload.d)?;
                self.on_ready(ready);
            }
            VoiceOpCode::SessionDescription => {
                let description: SessionDescription = decode(payload.d)?;
                self.on_session_description(&description);
            }
            VoiceOpCode::HeartbeatAck => {
                let ack: Option<HeartbeatAck> = serde_json::from_value(payload.d).ok();
                self.on_heartbeat_ack(ack, now);
            }
            VoiceOpCode::Resumed => {
                if self.state != State::Resuming {
                    tracing::debug!("Resumed without a resume; ignored");
                    return Ok(());
                }
                self.state = State::Active;
                self.session_failures = 0;
                self.resume_attempts = 0;
                self.push_event(CoreEvent::Resumed);
            }
            VoiceOpCode::Speaking => {
                let speaking: Speaking = decode(payload.d)?;
                if let Some(user_id) = speaking.user_id {
                    self.push_event(CoreEvent::UserSsrc {
                        user_id,
                        ssrc: speaking.ssrc,
                    });
                    self.push_event(CoreEvent::Speaking {
                        user_id,
                        ssrc: speaking.ssrc,
                        flags: speaking.speaking,
                    });
                }
            }
            VoiceOpCode::ClientFlags => {
                if let Ok(client) = serde_json::from_value::<ClientSsrc>(payload.d)
                    && client.audio_ssrc != 0
                {
                    self.push_event(CoreEvent::UserSsrc {
                        user_id: client.user_id,
                        ssrc: client.audio_ssrc,
                    });
                }
            }
            VoiceOpCode::ClientsConnect => {
                let connect: ClientsConnect = decode(payload.d)?;
                lock_e2ee(&self.e2ee).clients_connect(&connect.user_ids);
                self.push_event(CoreEvent::ClientsConnect(connect.user_ids));
            }
            VoiceOpCode::ClientDisconnect => {
                let disconnect: ClientDisconnect = decode(payload.d)?;
                lock_e2ee(&self.e2ee).client_disconnect(disconnect.user_id);
                self.push_event(CoreEvent::ClientDisconnect(disconnect.user_id));
            }
            VoiceOpCode::DavePrepareTransition
            | VoiceOpCode::DaveExecuteTransition
            | VoiceOpCode::DavePrepareEpoch => {
                let result = lock_e2ee(&self.e2ee).json_message(op.value(), &payload.d);
                self.send_e2ee(result);
            }
            _ => tracing::debug!(op = payload.op, "unexpected voice opcode"),
        }
        Ok(())
    }

    /// Reports a binary frame: a 2-byte big-endian sequence number, the opcode, the payload.
    ///
    /// # Errors
    ///
    /// Gives an error when the frame is shorter than 3 bytes.
    pub fn received_binary(&mut self, bytes: &[u8], _now: Instant) -> Result<(), FrameError> {
        let [high, low, op, payload @ ..] = bytes else {
            return Err(FrameError::ShortBinary(bytes.len()));
        };
        self.seq_ack = Some(i64::from(u16::from_be_bytes([*high, *low])));

        match VoiceOpCode::from_value(*op) {
            Some(
                VoiceOpCode::DaveMlsExternalSender
                | VoiceOpCode::DaveMlsProposals
                | VoiceOpCode::DaveMlsAnnounceCommitTransition
                | VoiceOpCode::DaveMlsWelcome,
            ) => {
                let result = lock_e2ee(&self.e2ee).binary_message(*op, payload);
                self.send_e2ee(result);
            }
            _ => tracing::debug!(op, "unexpected binary voice opcode"),
        }
        Ok(())
    }

    fn on_hello(&mut self, hello: &Hello, now: Instant) {
        let milliseconds = if hello.heartbeat_interval.is_finite() {
            hello.heartbeat_interval.clamp(100.0, 600_000.0)
        } else {
            30_000.0
        };
        let interval = Duration::from_secs_f64(milliseconds / 1000.0);
        self.heartbeat_interval = Some(interval);
        self.next_heartbeat = Some(now + interval);
        self.ack_pending = false;
        self.push_event(CoreEvent::Hello {
            heartbeat_interval: interval,
        });

        if self.identified {
            let resume = Resume::new(
                self.info.guild_id,
                self.info.session_id.clone(),
                self.info.token.clone(),
                self.seq_ack(),
            );
            self.send_text(encode(VoiceOpCode::Resume, &resume));
            self.state = State::Resuming;
        } else {
            let max_version = lock_e2ee(&self.e2ee).max_protocol_version();
            let identify = Identify::new(
                self.info.guild_id,
                self.info.user_id,
                self.info.session_id.clone(),
                self.info.token.clone(),
                max_version,
            );
            self.send_text(encode(VoiceOpCode::Identify, &identify));
            self.state = State::Identifying;
        }
    }

    fn on_ready(&mut self, ready: Ready) {
        if self.state != State::Identifying {
            tracing::warn!("Ready outside of a handshake; ignored");
            return;
        }
        let Some(mode) = EncryptionMode::select(&ready.modes) else {
            self.stop(VoiceFatal::NoEncryptionMode);
            return;
        };
        self.ssrc = Some(ready.ssrc);
        self.mode = Some(mode);
        self.state = State::AwaitingDiscovery;
        self.push_event(CoreEvent::Ready { ssrc: ready.ssrc });
        self.actions.push_back(VoiceAction::ConnectUdp {
            ip: ready.ip,
            port: ready.port,
            ssrc: ready.ssrc,
        });
    }

    fn on_session_description(&mut self, description: &SessionDescription) {
        // Each Session Description starts a new transport cipher at nonce 0. Accept only one
        // for each Select Protocol, so that a key is never used again from nonce 0.
        if self.state != State::SelectingProtocol {
            tracing::warn!("Session Description outside of a handshake; ignored");
            return;
        }
        let Some(mode) = EncryptionMode::from_name(&description.mode) else {
            self.stop(VoiceFatal::BadSessionDescription(format!(
                "unknown mode {}",
                description.mode
            )));
            return;
        };
        let Some(secret_key) = SecretKey::from_slice(&description.secret_key) else {
            self.stop(VoiceFatal::BadSessionDescription(format!(
                "the key has {} bytes",
                description.secret_key.len()
            )));
            return;
        };
        let Some(ssrc) = self.ssrc else {
            tracing::warn!("Session Description before Ready; ignored");
            return;
        };

        self.mode = Some(mode);
        self.state = State::Active;
        // Media runs only from here. A resume before this point gives a session with no media.
        self.identified = true;
        self.session_failures = 0;
        self.resume_attempts = 0;
        let result = lock_e2ee(&self.e2ee).session_description(
            description.dave_protocol_version,
            self.info.user_id,
            self.info.channel_id,
        );
        self.actions.push_back(VoiceAction::SessionReady {
            mode,
            secret_key,
            ssrc,
            dave_protocol_version: description.dave_protocol_version,
        });
        self.send_e2ee(result);
    }

    fn on_heartbeat_ack(&mut self, ack: Option<HeartbeatAck>, now: Instant) {
        if let Some(ack) = ack
            && ack.t != self.nonce
        {
            tracing::debug!(nonce = ack.t, "heartbeat acknowledgement for an old nonce");
        }
        self.ack_pending = false;
        if let Some(sent_at) = self.heartbeat_sent_at {
            let latency = now.saturating_duration_since(sent_at);
            self.latency = Some(latency);
            self.push_event(CoreEvent::HeartbeatAck { latency });
        }
    }

    fn send_heartbeat(&mut self, now: Instant) {
        self.nonce = self.nonce.wrapping_add(1);
        let heartbeat = Heartbeat::new(self.nonce, self.seq_ack());
        self.send_text(encode(VoiceOpCode::Heartbeat, &heartbeat));
        self.heartbeat_sent_at = Some(now);
        self.ack_pending = true;
        if let Some(interval) = self.heartbeat_interval {
            self.next_heartbeat = Some(now + interval);
        }
    }

    fn send_e2ee(&mut self, result: Result<Vec<E2eeOutgoing>, crate::e2ee::E2eeError>) {
        match result {
            Ok(messages) => {
                for message in messages {
                    self.send_e2ee_message(message);
                }
            }
            Err(error) => {
                tracing::warn!(%error, "the E2EE session reported an error");
                self.push_event(CoreEvent::E2eeError(error.to_string()));
            }
        }
    }

    fn send_e2ee_message(&mut self, message: E2eeOutgoing) {
        match message {
            E2eeOutgoing::Json { op, data } => {
                let payload = crate::protocol::SentPayload { op, d: &data };
                let text = serde_json::to_string(&payload).unwrap_or_default();
                self.send_text(text);
            }
            E2eeOutgoing::Binary { op, payload } => {
                let mut frame = Vec::with_capacity(payload.len() + 1);
                frame.push(op);
                frame.extend_from_slice(&payload);
                self.actions.push_back(VoiceAction::SendBinary(frame));
            }
        }
    }

    fn send_text(&mut self, text: String) {
        self.actions.push_back(VoiceAction::SendText(text));
    }

    fn push_event(&mut self, event: CoreEvent) {
        self.actions.push_back(VoiceAction::Event(event));
    }

    fn forget_session(&mut self) {
        self.identified = false;
        self.resume_attempts = 0;
        self.seq_ack = None;
        self.ssrc = None;
        self.mode = None;
    }

    fn reconnect(&mut self, kind: ReconnectKind) {
        let mut kind = kind;
        if kind == ReconnectKind::Resume {
            self.resume_attempts += 1;
            if !self.identified || self.resume_attempts > MAX_RESUME_ATTEMPTS {
                self.forget_session();
                kind = ReconnectKind::NewSession;
            }
        }

        let delay = match kind {
            ReconnectKind::Resume => resume_delay(self.resume_attempts),
            ReconnectKind::NewSession => {
                self.session_failures += 1;
                if self.session_failures > MAX_SESSION_FAILURES {
                    self.stop(VoiceFatal::TooManyFailures);
                    return;
                }
                Duration::from_secs(u64::from(self.session_failures.saturating_sub(1)))
            }
        };
        self.actions
            .push_back(VoiceAction::Reconnect { kind, delay });
    }

    fn stop(&mut self, error: VoiceFatal) {
        self.state = State::Stopped;
        self.next_heartbeat = None;
        self.actions.push_back(VoiceAction::Fatal(error));
    }
}

/// Gives the delay before a resume: none for the first attempt, then 1 s, 2 s, 4 s, and so on.
fn resume_delay(attempt: u32) -> Duration {
    if attempt <= 1 {
        return Duration::ZERO;
    }
    let seconds = 2_u64.saturating_pow(attempt - 2);
    Duration::from_secs(seconds).min(MAX_RESUME_DELAY)
}

fn decode<T: DeserializeOwned>(data: serde_json::Value) -> Result<T, serde_json::Error> {
    serde_json::from_value(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::e2ee::{E2eeError, NoE2ee};
    use serde_json::json;

    const GUILD: GuildId = GuildId::new_const(41_771_983_423_143_937);
    const CHANNEL: ChannelId = ChannelId::new_const(127_121_515_262_115_840);
    const USER: UserId = UserId::new_const(104_694_319_306_248_192);

    fn info() -> ConnectionInfo {
        ConnectionInfo::new(
            GUILD,
            CHANNEL,
            USER,
            "my_session_id".to_owned(),
            "my_token".to_owned(),
            "sweetwater-12345.discord.media:2048".to_owned(),
        )
    }

    fn shared(session: impl E2eeSession) -> SharedE2ee {
        Arc::new(Mutex::new(Box::new(session)))
    }

    fn drain(core: &mut VoiceCore) -> Vec<VoiceAction> {
        std::iter::from_fn(|| core.poll_action()).collect()
    }

    fn sent_json(actions: &[VoiceAction]) -> Vec<serde_json::Value> {
        actions
            .iter()
            .filter_map(|action| match action {
                VoiceAction::SendText(text) => Some(serde_json::from_str(text).expect("JSON")),
                _ => None,
            })
            .collect()
    }

    /// Connects a core up to the active state.
    fn active_core(e2ee: SharedE2ee, now: Instant) -> VoiceCore {
        let mut core = VoiceCore::new(info(), e2ee);
        core.connected(now);
        core.received_text(r#"{"op":8,"d":{"heartbeat_interval":41250.0}}"#, now)
            .expect("valid");
        core.received_text(
            r#"{"op":2,"d":{"ssrc":7,"ip":"127.0.0.1","port":1234,"modes":["aead_xchacha20_poly1305_rtpsize","aead_aes256_gcm_rtpsize"],"heartbeat_interval":1}}"#,
            now,
        )
        .expect("valid");
        core.ip_discovered("1.2.3.4".to_owned(), 5000, now);
        let key: Vec<u8> = (0..32).collect();
        let description = json!({"op":4,"d":{"mode":"aead_aes256_gcm_rtpsize","secret_key":key,"dave_protocol_version":0}});
        core.received_text(&description.to_string(), now)
            .expect("valid");
        assert!(core.is_active());
        drain(&mut core);
        core
    }

    #[test]
    fn handshake() {
        let now = Instant::now();
        let mut core = VoiceCore::new(info(), shared(NoE2ee));
        core.connected(now);
        core.received_text(r#"{"op":8,"d":{"heartbeat_interval":41250.0}}"#, now)
            .expect("valid");
        let actions = drain(&mut core);
        assert_eq!(
            actions[0],
            VoiceAction::Event(CoreEvent::Hello {
                heartbeat_interval: Duration::from_millis(41_250)
            })
        );
        assert_eq!(
            sent_json(&actions),
            vec![json!({"op":0,"d":{
                "server_id":"41771983423143937",
                "user_id":"104694319306248192",
                "session_id":"my_session_id",
                "token":"my_token",
                "max_dave_protocol_version":0
            }})]
        );

        core.received_text(
            r#"{"op":2,"d":{"ssrc":7,"ip":"10.0.0.1","port":1234,"modes":["aead_xchacha20_poly1305_rtpsize","aead_aes256_gcm_rtpsize"]}}"#,
            now,
        )
        .expect("valid");
        let actions = drain(&mut core);
        assert!(actions.contains(&VoiceAction::ConnectUdp {
            ip: "10.0.0.1".to_owned(),
            port: 1234,
            ssrc: 7
        }));
        assert_eq!(core.mode(), Some(EncryptionMode::Aes256GcmRtpSize));

        core.ip_discovered("1.2.3.4".to_owned(), 5000, now);
        assert_eq!(
            sent_json(&drain(&mut core)),
            vec![json!({"op":1,"d":{"protocol":"udp","data":{
                "address":"1.2.3.4","port":5000,"mode":"aead_aes256_gcm_rtpsize"
            }}})]
        );

        let key: Vec<u8> = (0..32).collect();
        let description = json!({"op":4,"d":{"mode":"aead_aes256_gcm_rtpsize","secret_key":key,"dave_protocol_version":0}});
        core.received_text(&description.to_string(), now)
            .expect("valid");
        let actions = drain(&mut core);
        let expected_key = SecretKey::from_slice(&key).expect("32 bytes");
        assert_eq!(
            actions,
            vec![VoiceAction::SessionReady {
                mode: EncryptionMode::Aes256GcmRtpSize,
                secret_key: expected_key,
                ssrc: 7,
                dave_protocol_version: 0,
            }]
        );
        assert!(core.is_active());

        assert!(core.set_speaking(SpeakingFlags::MICROPHONE));
        assert_eq!(
            sent_json(&drain(&mut core)),
            vec![json!({"op":5,"d":{"speaking":1,"delay":0,"ssrc":7}})]
        );
    }

    #[test]
    fn xchacha_when_aes_is_not_offered() {
        let now = Instant::now();
        let mut core = VoiceCore::new(info(), shared(NoE2ee));
        core.connected(now);
        core.received_text(r#"{"op":8,"d":{"heartbeat_interval":1000}}"#, now)
            .expect("valid");
        core.received_text(
            r#"{"op":2,"d":{"ssrc":7,"ip":"10.0.0.1","port":1,"modes":["aead_xchacha20_poly1305_rtpsize","xsalsa20_poly1305"]}}"#,
            now,
        )
        .expect("valid");
        assert_eq!(core.mode(), Some(EncryptionMode::XChaCha20Poly1305RtpSize));
    }

    #[test]
    fn heartbeat_has_nonce_and_seq_ack() {
        let start = Instant::now();
        let mut core = VoiceCore::new(info(), shared(NoE2ee));
        core.connected(start);
        core.received_text(r#"{"op":8,"d":{"heartbeat_interval":1000}}"#, start)
            .expect("valid");
        drain(&mut core);

        core.tick(start + Duration::from_millis(999));
        assert!(drain(&mut core).is_empty());

        let first = start + Duration::from_secs(1);
        assert_eq!(core.next_deadline(), Some(first));
        core.tick(first);
        assert_eq!(
            sent_json(&drain(&mut core)),
            vec![json!({"op":3,"d":{"t":1,"seq_ack":-1}})]
        );

        core.received_text(r#"{"op":6,"d":{"t":1}}"#, first + Duration::from_millis(30))
            .expect("valid");
        assert_eq!(
            drain(&mut core),
            vec![VoiceAction::Event(CoreEvent::HeartbeatAck {
                latency: Duration::from_millis(30)
            })]
        );

        core.received_text(
            r#"{"op":5,"d":{"user_id":"9","ssrc":110,"speaking":0},"seq":10}"#,
            first,
        )
        .expect("valid");
        assert_eq!(core.seq_ack(), 10);
        core.received_binary(&[0x00, 0x0B, 99, 1, 2], first)
            .expect("valid");
        assert_eq!(core.seq_ack(), 11);
        drain(&mut core);

        core.tick(start + Duration::from_secs(2));
        assert_eq!(
            sent_json(&drain(&mut core)),
            vec![json!({"op":3,"d":{"t":2,"seq_ack":11}})]
        );
    }

    #[test]
    fn missing_ack_resumes() {
        let now = Instant::now();
        let mut core = active_core(shared(NoE2ee), now);
        core.tick(now + Duration::from_millis(41_250));
        drain(&mut core);
        core.tick(now + Duration::from_millis(82_500));
        let actions = drain(&mut core);
        assert_eq!(
            actions,
            vec![
                VoiceAction::Event(CoreEvent::Zombie),
                VoiceAction::Reconnect {
                    kind: ReconnectKind::Resume,
                    delay: Duration::ZERO
                }
            ]
        );
    }

    #[test]
    fn resume_sends_seq_ack() {
        let now = Instant::now();
        let mut core = active_core(shared(NoE2ee), now);
        core.received_text(
            r#"{"op":5,"d":{"user_id":"9","ssrc":3,"speaking":1},"seq":4}"#,
            now,
        )
        .expect("valid");
        drain(&mut core);

        core.closed(Some(4015), now);
        assert_eq!(
            drain(&mut core),
            vec![VoiceAction::Reconnect {
                kind: ReconnectKind::Resume,
                delay: Duration::ZERO
            }]
        );

        core.connected(now);
        core.received_text(r#"{"op":8,"d":{"heartbeat_interval":1000}}"#, now)
            .expect("valid");
        assert_eq!(
            sent_json(&drain(&mut core)),
            vec![json!({"op":7,"d":{
                "server_id":"41771983423143937",
                "session_id":"my_session_id",
                "token":"my_token",
                "seq_ack":4
            }})]
        );
        assert!(!core.is_active());
        core.received_text(r#"{"op":9,"d":null}"#, now)
            .expect("valid");
        assert!(core.is_active());
        assert_eq!(
            drain(&mut core),
            vec![VoiceAction::Event(CoreEvent::Resumed)]
        );
    }

    #[test]
    fn close_codes() {
        let now = Instant::now();
        for code in [4014_u16, 4021, 4022] {
            let mut core = active_core(shared(NoE2ee), now);
            core.closed(Some(code), now);
            assert_eq!(
                drain(&mut core),
                vec![VoiceAction::Fatal(VoiceFatal::Closed(
                    VoiceCloseCode::from_value(code)
                ))]
            );
            assert!(core.is_stopped());
        }

        for code in [4006_u16, 4009] {
            let mut core = active_core(shared(NoE2ee), now);
            core.closed(Some(code), now);
            assert_eq!(
                drain(&mut core),
                vec![VoiceAction::Reconnect {
                    kind: ReconnectKind::NewSession,
                    delay: Duration::ZERO
                }]
            );
            assert!(!core.will_resume());
            assert_eq!(core.seq_ack(), -1);
        }

        let mut core = active_core(shared(NoE2ee), now);
        core.closed(None, now);
        assert_eq!(
            drain(&mut core),
            vec![VoiceAction::Reconnect {
                kind: ReconnectKind::Resume,
                delay: Duration::ZERO
            }]
        );
    }

    #[test]
    fn repeated_session_failures_stop() {
        let now = Instant::now();
        let mut core = VoiceCore::new(info(), shared(NoE2ee));
        for _ in 0..MAX_SESSION_FAILURES {
            core.connected(now);
            core.closed(Some(4006), now);
        }
        core.connected(now);
        core.closed(Some(4006), now);
        let actions = drain(&mut core);
        assert_eq!(
            actions.last(),
            Some(&VoiceAction::Fatal(VoiceFatal::TooManyFailures))
        );
    }

    #[test]
    fn no_supported_mode_is_fatal() {
        let now = Instant::now();
        let mut core = VoiceCore::new(info(), shared(NoE2ee));
        core.connected(now);
        core.received_text(r#"{"op":8,"d":{"heartbeat_interval":1000}}"#, now)
            .expect("valid");
        core.received_text(
            r#"{"op":2,"d":{"ssrc":7,"ip":"10.0.0.1","port":1,"modes":["xsalsa20_poly1305"]}}"#,
            now,
        )
        .expect("valid");
        assert_eq!(
            drain(&mut core).last(),
            Some(&VoiceAction::Fatal(VoiceFatal::NoEncryptionMode))
        );
    }

    /// An E2EE session that records what it receives.
    #[derive(Default)]
    struct Recorder {
        log: Arc<Mutex<Vec<String>>>,
    }

    impl E2eeSession for Recorder {
        fn max_protocol_version(&self) -> u16 {
            1
        }

        fn session_description(
            &mut self,
            protocol_version: u16,
            own_user_id: UserId,
            channel_id: ChannelId,
        ) -> Result<Vec<E2eeOutgoing>, E2eeError> {
            self.log.lock().expect("lock").push(format!(
                "session {protocol_version} {own_user_id} {channel_id}"
            ));
            Ok(vec![E2eeOutgoing::Binary {
                op: 26,
                payload: vec![1, 2, 3],
            }])
        }

        fn clients_connect(&mut self, user_ids: &[UserId]) {
            self.log
                .lock()
                .expect("lock")
                .push(format!("connect {user_ids:?}"));
        }

        fn client_disconnect(&mut self, user_id: UserId) {
            self.log
                .lock()
                .expect("lock")
                .push(format!("disconnect {user_id}"));
        }

        fn json_message(
            &mut self,
            op: u8,
            data: &serde_json::Value,
        ) -> Result<Vec<E2eeOutgoing>, E2eeError> {
            self.log
                .lock()
                .expect("lock")
                .push(format!("json {op} {data}"));
            if op == 21 {
                return Ok(vec![E2eeOutgoing::Json {
                    op: 23,
                    data: json!({"transition_id": data["transition_id"]}),
                }]);
            }
            Ok(Vec::new())
        }

        fn binary_message(
            &mut self,
            op: u8,
            payload: &[u8],
        ) -> Result<Vec<E2eeOutgoing>, E2eeError> {
            self.log
                .lock()
                .expect("lock")
                .push(format!("binary {op} {payload:?}"));
            if op == 29 {
                return Err(E2eeError::Protocol("bad commit".to_owned()));
            }
            Ok(Vec::new())
        }

        fn encrypt(&mut self, opus_frame: &[u8]) -> Result<Vec<u8>, E2eeError> {
            Ok(opus_frame.to_vec())
        }

        fn decrypt(&mut self, _sender: UserId, frame: &[u8]) -> Result<Vec<u8>, E2eeError> {
            Ok(frame.to_vec())
        }
    }

    #[test]
    fn dave_routing() {
        let now = Instant::now();
        let recorder = Recorder::default();
        let log = Arc::clone(&recorder.log);
        let mut core = VoiceCore::new(info(), shared(recorder));
        core.connected(now);
        core.received_text(r#"{"op":8,"d":{"heartbeat_interval":1000}}"#, now)
            .expect("valid");
        let identify = sent_json(&drain(&mut core));
        assert_eq!(identify[0]["d"]["max_dave_protocol_version"], 1);

        core.received_text(
            r#"{"op":2,"d":{"ssrc":7,"ip":"10.0.0.1","port":1,"modes":["aead_xchacha20_poly1305_rtpsize"]}}"#,
            now,
        )
        .expect("valid");
        core.ip_discovered("1.2.3.4".to_owned(), 5000, now);
        drain(&mut core);
        let key = vec![0_u8; 32];
        let description = json!({"op":4,"d":{"mode":"aead_xchacha20_poly1305_rtpsize","secret_key":key,"dave_protocol_version":1},"seq":1});
        core.received_text(&description.to_string(), now)
            .expect("valid");
        let actions = drain(&mut core);
        assert_eq!(
            actions.last(),
            Some(&VoiceAction::SendBinary(vec![26, 1, 2, 3]))
        );

        core.received_text(r#"{"op":11,"d":{"user_ids":["5","6"]},"seq":2}"#, now)
            .expect("valid");
        core.received_text(r#"{"op":13,"d":{"user_id":"5"},"seq":3}"#, now)
            .expect("valid");
        core.received_text(
            r#"{"op":21,"d":{"protocol_version":1,"transition_id":3},"seq":4}"#,
            now,
        )
        .expect("valid");
        let actions = drain(&mut core);
        assert!(
            actions.contains(&VoiceAction::Event(CoreEvent::ClientsConnect(vec![
                UserId::new_const(5),
                UserId::new_const(6)
            ])))
        );
        assert!(
            actions.contains(&VoiceAction::Event(CoreEvent::ClientDisconnect(
                UserId::new_const(5)
            )))
        );
        assert_eq!(
            sent_json(&actions),
            vec![json!({"op":23,"d":{"transition_id":3}})]
        );

        core.received_binary(&[0x00, 0x05, 25, 9, 9], now)
            .expect("valid");
        core.received_binary(&[0x00, 0x06, 29, 0, 1, 7], now)
            .expect("valid");
        core.received_binary(&[0x00, 0x07, 26, 1], now)
            .expect("valid");
        assert_eq!(core.seq_ack(), 7);
        let actions = drain(&mut core);
        assert_eq!(
            actions,
            vec![VoiceAction::Event(CoreEvent::E2eeError(
                "E2EE protocol error: bad commit".to_owned()
            ))]
        );
        assert!(core.received_binary(&[0, 1], now).is_err());

        let log = log.lock().expect("lock").clone();
        assert_eq!(
            log,
            vec![
                format!("session 1 {USER} {CHANNEL}"),
                "connect [Id(5), Id(6)]".to_owned(),
                "disconnect 5".to_owned(),
                r#"json 21 {"protocol_version":1,"transition_id":3}"#.to_owned(),
                "binary 25 [9, 9]".to_owned(),
                "binary 29 [0, 1, 7]".to_owned(),
            ]
        );
    }

    fn session_description(key: &[u8]) -> String {
        json!({"op":4,"d":{"mode":"aead_aes256_gcm_rtpsize","secret_key":key,"dave_protocol_version":0}})
            .to_string()
    }

    /// A new transport cipher starts at nonce 0. A second Session Description with the same
    /// key must not start one, or the nonces repeat.
    #[test]
    fn second_session_description_is_ignored() {
        let now = Instant::now();
        let mut core = active_core(shared(NoE2ee), now);
        let key: Vec<u8> = (0..32).collect();
        core.received_text(&session_description(&key), now)
            .expect("valid");
        assert!(drain(&mut core).is_empty());

        // A second Ready in the same session does not start a new handshake either.
        core.received_text(
            r#"{"op":2,"d":{"ssrc":8,"ip":"127.0.0.1","port":1,"modes":["aead_aes256_gcm_rtpsize"]}}"#,
            now,
        )
        .expect("valid");
        assert!(drain(&mut core).is_empty());
        core.ip_discovered("1.2.3.4".to_owned(), 5000, now);
        core.received_text(&session_description(&key), now)
            .expect("valid");
        assert!(drain(&mut core).is_empty());
        assert_eq!(core.ssrc(), Some(7));
        assert!(core.is_active());
    }

    /// Without a Session Description, no media runs. A resume would report a ready call
    /// with no media, so the core identifies again.
    #[test]
    fn close_before_session_description_starts_a_new_session() {
        let now = Instant::now();
        let mut core = VoiceCore::new(info(), shared(NoE2ee));
        core.connected(now);
        core.received_text(r#"{"op":8,"d":{"heartbeat_interval":1000}}"#, now)
            .expect("valid");
        core.received_text(
            r#"{"op":2,"d":{"ssrc":7,"ip":"10.0.0.1","port":1,"modes":["aead_aes256_gcm_rtpsize"]},"seq":1}"#,
            now,
        )
        .expect("valid");
        drain(&mut core);

        core.closed(None, now);
        assert_eq!(
            drain(&mut core),
            vec![VoiceAction::Reconnect {
                kind: ReconnectKind::NewSession,
                delay: Duration::ZERO
            }]
        );
        assert!(!core.will_resume());
        assert_eq!(core.seq_ack(), -1);

        core.connected(now);
        core.received_text(r#"{"op":8,"d":{"heartbeat_interval":1000}}"#, now)
            .expect("valid");
        let sent = sent_json(&drain(&mut core));
        assert_eq!(sent[0]["op"], 0);

        // A Resumed that the core did not ask for does not make the session active.
        core.received_text(r#"{"op":9,"d":null}"#, now)
            .expect("valid");
        assert!(!core.is_active());
        assert!(drain(&mut core).is_empty());
    }

    /// A server that accepts each resume and then closes the connection again.
    #[test]
    fn resume_attempts_have_a_delay_and_a_limit() {
        let now = Instant::now();
        let mut core = active_core(shared(NoE2ee), now);
        let mut delays = Vec::new();
        for _ in 0..MAX_RESUME_ATTEMPTS {
            core.closed(Some(4015), now);
            let actions = drain(&mut core);
            let [VoiceAction::Reconnect { kind, delay }] = actions.as_slice() else {
                panic!("one reconnect expected, got {actions:?}");
            };
            assert_eq!(*kind, ReconnectKind::Resume);
            delays.push(delay.as_secs());
            core.connected(now);
            core.received_text(r#"{"op":8,"d":{"heartbeat_interval":1000}}"#, now)
                .expect("valid");
            assert_eq!(sent_json(&drain(&mut core))[0]["op"], 7);
        }
        assert_eq!(delays, vec![0, 1, 2, 4, 8]);

        core.closed(Some(4015), now);
        assert_eq!(
            drain(&mut core),
            vec![VoiceAction::Reconnect {
                kind: ReconnectKind::NewSession,
                delay: Duration::ZERO
            }]
        );
        assert!(!core.will_resume());
    }

    /// A resume that succeeds sets the count of resume attempts to zero.
    #[test]
    fn resumed_clears_the_resume_attempts() {
        let now = Instant::now();
        let mut core = active_core(shared(NoE2ee), now);
        for _ in 0..3 {
            core.closed(Some(4015), now);
            drain(&mut core);
            core.connected(now);
            core.received_text(r#"{"op":8,"d":{"heartbeat_interval":1000}}"#, now)
                .expect("valid");
        }
        core.received_text(r#"{"op":9,"d":null}"#, now)
            .expect("valid");
        assert!(core.is_active());
        drain(&mut core);

        core.closed(Some(4015), now);
        assert_eq!(
            drain(&mut core),
            vec![VoiceAction::Reconnect {
                kind: ReconnectKind::Resume,
                delay: Duration::ZERO
            }]
        );
    }

    #[test]
    fn url() {
        assert_eq!(
            info().websocket_url(),
            "wss://sweetwater-12345.discord.media:2048/?v=8"
        );
        let mut local = info();
        local.endpoint = "ws://127.0.0.1:9000".to_owned();
        assert_eq!(local.websocket_url(), "ws://127.0.0.1:9000/?v=8");
    }
}
