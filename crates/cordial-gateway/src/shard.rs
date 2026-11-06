//! The Tokio driver of one shard.

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cordial_http::Token;
use cordial_model::gateway::event::Event;
use cordial_model::gateway::{
    Capabilities, EventTypes, IdentifyProperties, Intents, UpdatePresence,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode as WsCloseCode;

use crate::compression::{Compression, Decompressor};
use crate::core::{
    Action, Command, CommandError, CoreConfig, FatalError, Lifecycle, ReconnectKind, Session,
    ShardCore,
};
use crate::queue::{IdentifyQueue, LocalIdentifyQueue};

/// The default gateway URL. `GET /gateway/bot` gives the current one.
pub const DEFAULT_GATEWAY_URL: &str = "wss://gateway.discord.gg";

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// The settings of a shard.
#[derive(Clone)]
#[non_exhaustive]
pub struct ShardConfig {
    /// The bot token.
    pub token: Token,
    /// The intents.
    pub intents: Intents,
    /// The shard ID and the shard count.
    pub shard: [u32; 2],
    /// The gateway URL, from `GET /gateway/bot`.
    pub gateway_url: String,
    /// The transport compression.
    pub compression: Compression,
    /// The member count above which Discord does not send offline members (50 to 250).
    pub large_threshold: Option<u8>,
    /// The initial presence.
    pub presence: Option<UpdatePresence>,
    /// The capabilities.
    pub capabilities: Capabilities,
    /// The event types to decode. Other events are dropped without decode, which saves CPU.
    /// `READY`, `RESUMED`, and `RATE_LIMITED` are always decoded.
    pub event_types: EventTypes,
    /// The identify queue.
    pub identify_queue: Arc<dyn IdentifyQueue>,
    /// A session to resume on the first connection, for example after a process restart.
    pub session: Option<Session>,
    /// The capacity of the event channel. When the consumer is slow, the shard waits.
    pub event_buffer: usize,
}

impl fmt::Debug for ShardConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ShardConfig")
            .field("intents", &self.intents)
            .field("shard", &self.shard)
            .field("gateway_url", &self.gateway_url)
            .field("compression", &self.compression)
            .finish_non_exhaustive()
    }
}

impl ShardConfig {
    /// Makes settings for shard 0 of 1 with all event types.
    #[must_use]
    pub fn new(token: Token, intents: Intents) -> Self {
        Self {
            token,
            intents,
            shard: [0, 1],
            gateway_url: DEFAULT_GATEWAY_URL.to_owned(),
            compression: Compression::default(),
            large_threshold: None,
            presence: None,
            capabilities: Capabilities::empty(),
            event_types: EventTypes::all(),
            identify_queue: Arc::new(LocalIdentifyQueue::default()),
            session: None,
            event_buffer: 1024,
        }
    }

    /// Sets the shard ID and the shard count.
    #[must_use]
    pub fn shard(mut self, id: u32, count: u32) -> Self {
        self.shard = [id, count];
        self
    }

    /// Sets the gateway URL.
    #[must_use]
    pub fn gateway_url(mut self, url: impl Into<String>) -> Self {
        self.gateway_url = url.into();
        self
    }

    /// Sets the transport compression.
    #[must_use]
    pub fn compression(mut self, compression: Compression) -> Self {
        self.compression = compression;
        self
    }

    /// Sets the initial presence.
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

    /// Sets the identify queue.
    #[must_use]
    pub fn identify_queue(mut self, queue: Arc<dyn IdentifyQueue>) -> Self {
        self.identify_queue = queue;
        self
    }

    /// Sets a session to resume on the first connection.
    #[must_use]
    pub fn resume(mut self, session: Session) -> Self {
        self.session = Some(session);
        self
    }

    /// Sets the capabilities.
    #[must_use]
    pub fn capabilities(mut self, capabilities: Capabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Sets the member count above which Discord does not send offline members.
    #[must_use]
    pub fn large_threshold(mut self, threshold: u8) -> Self {
        self.large_threshold = Some(threshold.clamp(50, 250));
        self
    }

    fn core_config(&self) -> CoreConfig {
        let mut config = CoreConfig::new(self.token.secret(), self.intents);
        config.shard = self.shard;
        config.large_threshold = self.large_threshold;
        config.presence.clone_from(&self.presence);
        config.capabilities = self.capabilities;
        config.properties = IdentifyProperties::new("cordial");
        config
    }
}

/// An event of a shard.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum ShardEvent {
    /// A dispatch event.
    Dispatch(Box<Event>),
    /// A lifecycle change of the session.
    Lifecycle(Lifecycle),
    /// The shard connects to the gateway.
    Connecting {
        /// Tells if the shard resumes a session.
        resume: bool,
    },
    /// The WebSocket connection is open.
    Connected,
    /// The connection closed.
    Disconnected {
        /// The close code, if one arrived.
        code: Option<u16>,
    },
    /// An event could not be decoded. The shard continues.
    DecodeError {
        /// The event name.
        name: String,
        /// The error.
        error: String,
    },
    /// The shard stopped because of a fatal error.
    Fatal(FatalError),
}

/// An event with the ID of its shard.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ShardMessage {
    /// The shard ID.
    pub shard_id: u32,
    /// The event.
    pub event: ShardEvent,
}

/// How to close a shard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CloseMode {
    /// End the session. The bot goes offline.
    End,
    /// Keep the session for a short time, so that a new process can resume it.
    Resumable,
}

/// An error of a shard handle.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ShardError {
    /// The command was not accepted.
    #[error(transparent)]
    Command(#[from] CommandError),
    /// The shard has stopped.
    #[error("the shard has stopped")]
    Stopped,
}

enum Request {
    Command(Command, oneshot::Sender<Result<(), CommandError>>),
    Close(CloseMode, oneshot::Sender<Option<Session>>),
}

#[derive(Debug, Default)]
struct Status {
    latency: Option<Duration>,
    session: Option<Session>,
    active: bool,
}

/// A handle to a running shard. It is cheap to clone.
#[derive(Clone)]
pub struct ShardHandle {
    id: u32,
    requests: mpsc::Sender<Request>,
    status: Arc<Mutex<Status>>,
}

impl fmt::Debug for ShardHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ShardHandle")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl ShardHandle {
    /// Gives the shard ID.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Sends a command. It goes out when the send limit permits.
    ///
    /// # Errors
    ///
    /// Gives an error when the command is not valid now or the shard has stopped.
    pub async fn send(&self, command: Command) -> Result<(), ShardError> {
        let (reply, answer) = oneshot::channel();
        self.requests
            .send(Request::Command(command, reply))
            .await
            .map_err(|_| ShardError::Stopped)?;
        answer
            .await
            .map_err(|_| ShardError::Stopped)?
            .map_err(ShardError::from)
    }

    /// Closes the shard. With [`CloseMode::Resumable`], gives the session to resume later.
    pub async fn close(&self, mode: CloseMode) -> Option<Session> {
        let (reply, answer) = oneshot::channel();
        self.requests.send(Request::Close(mode, reply)).await.ok()?;
        answer.await.ok().flatten()
    }

    /// Gives the last heartbeat latency.
    #[must_use]
    pub fn latency(&self) -> Option<Duration> {
        lock(&self.status).latency
    }

    /// Gives the current session.
    #[must_use]
    pub fn session(&self) -> Option<Session> {
        lock(&self.status).session.clone()
    }

    /// Tells if the session receives events now.
    #[must_use]
    pub fn is_active(&self) -> bool {
        lock(&self.status).active
    }

    /// Tells if the shard task has stopped.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.requests.is_closed()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A shard: one gateway connection.
#[derive(Debug)]
pub struct Shard;

impl Shard {
    /// Starts a shard on the Tokio runtime. Gives a handle and the event receiver.
    #[must_use]
    pub fn start(config: ShardConfig) -> (ShardHandle, mpsc::Receiver<ShardMessage>) {
        let (events, receiver) = mpsc::channel(config.event_buffer.max(1));
        let handle = Self::start_with_sender(config, events);
        (handle, receiver)
    }

    /// Starts a shard that sends its events to an existing channel (for the shard manager).
    #[must_use]
    pub fn start_with_sender(
        config: ShardConfig,
        events: mpsc::Sender<ShardMessage>,
    ) -> ShardHandle {
        let (requests, request_receiver) = mpsc::channel(64);
        let status = Arc::new(Mutex::new(Status {
            session: config.session.clone(),
            ..Status::default()
        }));
        let handle = ShardHandle {
            id: config.shard[0],
            requests,
            status: Arc::clone(&status),
        };

        let driver = Driver::new(config, events, request_receiver, status);
        tokio::spawn(driver.run());
        handle
    }
}

/// What the connection loop does next.
enum Next {
    Reconnect(Duration),
    Stop,
}

struct Driver {
    config: ShardConfig,
    core: ShardCore,
    events: mpsc::Sender<ShardMessage>,
    requests: mpsc::Receiver<Request>,
    status: Arc<Mutex<Status>>,
    failures: u32,
}

impl Driver {
    fn new(
        config: ShardConfig,
        events: mpsc::Sender<ShardMessage>,
        requests: mpsc::Receiver<Request>,
        status: Arc<Mutex<Status>>,
    ) -> Self {
        let now = std::time::Instant::now();
        let core = match config.session.clone() {
            Some(session) => ShardCore::with_session(config.core_config(), session, now),
            None => ShardCore::new(config.core_config(), now),
        };
        Self {
            config,
            core,
            events,
            requests,
            status,
            failures: 0,
        }
    }

    async fn run(mut self) {
        let shard_id = self.config.shard[0];
        loop {
            let resume = self.core.will_resume();
            if !self.emit(ShardEvent::Connecting { resume }).await {
                return;
            }

            let url = self.connection_url();
            let socket = match tokio::time::timeout(
                Duration::from_secs(30),
                tokio_tungstenite::connect_async(&url),
            )
            .await
            {
                Ok(Ok((socket, _))) => socket,
                Ok(Err(error)) => {
                    tracing::warn!(shard = shard_id, %error, "gateway connection failed");
                    self.wait_after_failure().await;
                    continue;
                }
                Err(_) => {
                    tracing::warn!(shard = shard_id, "gateway connection timed out");
                    self.wait_after_failure().await;
                    continue;
                }
            };

            match self.run_connection(socket).await {
                Next::Reconnect(delay) => {
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                }
                Next::Stop => return,
            }
        }
    }

    fn connection_url(&self) -> String {
        let base = self
            .core
            .resume_url()
            .unwrap_or(&self.config.gateway_url)
            .trim_end_matches('/');
        let mut url = format!("{base}/?v={}&encoding=json", cordial_model::API_VERSION);
        if let Some(compression) = self.config.compression.query_value() {
            url.push_str("&compress=");
            url.push_str(compression);
        }
        url
    }

    async fn wait_after_failure(&mut self) {
        self.failures = self.failures.saturating_add(1);
        let seconds = 2_u64.saturating_pow(self.failures.min(6)).min(60);
        tokio::time::sleep(Duration::from_secs(seconds)).await;
    }

    async fn run_connection(&mut self, mut socket: Socket) -> Next {
        let mut decompressor = match Decompressor::new(self.config.compression) {
            Ok(decompressor) => decompressor,
            Err(error) => {
                tracing::error!(%error, "cannot start decompression");
                return Next::Stop;
            }
        };

        self.core.connected(std::time::Instant::now());
        if !self.emit(ShardEvent::Connected).await {
            return Next::Stop;
        }

        let mut identify_permit: Option<cordial_http::transport::BoxFuture<'static, ()>> = None;

        loop {
            if let Some(next) = self.do_actions(&mut socket, &mut identify_permit).await {
                return next;
            }

            let deadline = self.core.next_deadline().map_or_else(
                || Instant::now() + Duration::from_secs(3600),
                Instant::from_std,
            );

            tokio::select! {
                frame = socket.next() => {
                    let now = std::time::Instant::now();
                    match frame {
                        Some(Ok(Message::Text(text))) => self.receive(text.as_str(), now),
                        Some(Ok(Message::Binary(bytes))) => match decompressor.push(&bytes) {
                            Ok(Some(text)) => self.receive(&text, now),
                            Ok(None) => {}
                            Err(error) => {
                                tracing::warn!(%error, "decompression failed; reconnecting");
                                self.core.closed(None, now);
                            }
                        },
                        Some(Ok(Message::Close(frame))) => {
                            let code = frame.map(|frame| u16::from(frame.code));
                            tracing::info!(shard = self.config.shard[0], ?code, "gateway closed the connection");
                            if !self.emit(ShardEvent::Disconnected { code }).await {
                                return Next::Stop;
                            }
                            self.core.closed(code, now);
                        }
                        Some(Ok(_)) => {}
                        Some(Err(error)) => {
                            tracing::warn!(%error, "gateway connection error");
                            if !self.emit(ShardEvent::Disconnected { code: None }).await {
                                return Next::Stop;
                            }
                            self.core.closed(None, now);
                        }
                        None => {
                            if !self.emit(ShardEvent::Disconnected { code: None }).await {
                                return Next::Stop;
                            }
                            self.core.closed(None, now);
                        }
                    }
                }
                () = tokio::time::sleep_until(deadline) => {
                    self.core.tick(std::time::Instant::now());
                }
                () = async { identify_permit.as_mut().expect("checked by the guard").await }, if identify_permit.is_some() => {
                    identify_permit = None;
                    self.core.identify(std::time::Instant::now());
                }
                request = self.requests.recv() => match request {
                    Some(Request::Command(command, reply)) => {
                        let result = self.core.send_command(&command, std::time::Instant::now());
                        let _ = reply.send(result);
                    }
                    Some(Request::Close(mode, reply)) => {
                        let session = self.close(&mut socket, mode).await;
                        let _ = reply.send(session);
                        return Next::Stop;
                    }
                    None => {
                        self.close(&mut socket, CloseMode::End).await;
                        return Next::Stop;
                    }
                },
            }
        }
    }

    fn receive(&mut self, text: &str, now: std::time::Instant) {
        if let Err(error) = self.core.received(text, now) {
            tracing::warn!(%error, "the gateway sent a payload that is not valid");
        }
    }

    /// Runs the queued actions of the core. Gives `Some` when the connection must end.
    async fn do_actions(
        &mut self,
        socket: &mut Socket,
        identify_permit: &mut Option<cordial_http::transport::BoxFuture<'static, ()>>,
    ) -> Option<Next> {
        while let Some(action) = self.core.poll_action() {
            match action {
                Action::Send(text) => {
                    if let Err(error) = socket.send(Message::Text(text.into())).await {
                        tracing::warn!(%error, "cannot send to the gateway");
                        self.core.closed(None, std::time::Instant::now());
                    }
                }
                Action::RequestIdentify => {
                    *identify_permit =
                        Some(self.config.identify_queue.acquire(self.config.shard[0]));
                }
                Action::Reconnect {
                    kind,
                    close_code,
                    delay,
                } => {
                    tracing::info!(shard = self.config.shard[0], ?kind, "reconnecting");
                    let frame = CloseFrame {
                        code: WsCloseCode::from(close_code),
                        reason: "".into(),
                    };
                    let _ = tokio::time::timeout(Duration::from_secs(5), socket.close(Some(frame)))
                        .await;
                    self.update_status();
                    let delay = if kind == ReconnectKind::Identify {
                        delay.max(Duration::from_millis(100))
                    } else {
                        delay
                    };
                    return Some(Next::Reconnect(delay));
                }
                Action::Dispatch { name, data } => {
                    if !self.dispatch(&name, &data).await {
                        return Some(Next::Stop);
                    }
                }
                Action::Lifecycle(lifecycle) => {
                    if matches!(lifecycle, Lifecycle::Ready { .. } | Lifecycle::Resumed) {
                        self.failures = 0;
                    }
                    self.update_status();
                    if !self.emit(ShardEvent::Lifecycle(lifecycle)).await {
                        return Some(Next::Stop);
                    }
                }
                Action::Fatal(error) => {
                    tracing::error!(shard = self.config.shard[0], %error, "the shard stops");
                    self.update_status();
                    let _ = self.emit(ShardEvent::Fatal(error)).await;
                    return Some(Next::Stop);
                }
            }
        }
        None
    }

    async fn dispatch(&mut self, name: &str, data: &serde_json::value::RawValue) -> bool {
        let always = matches!(name, "READY" | "RESUMED" | "RATE_LIMITED");
        let wanted = always
            || cordial_model::gateway::EventType::from_name(name)
                .is_none_or(|kind| self.config.event_types.contains(kind));
        if !wanted {
            return true;
        }

        match Event::decode(name, data) {
            Ok(event) => self.emit(ShardEvent::Dispatch(Box::new(event))).await,
            Err(error) => {
                tracing::warn!(event = name, %error, "cannot decode a gateway event");
                self.emit(ShardEvent::DecodeError {
                    name: name.to_owned(),
                    error: error.to_string(),
                })
                .await
            }
        }
    }

    async fn close(&mut self, socket: &mut Socket, mode: CloseMode) -> Option<Session> {
        let code = match mode {
            CloseMode::End => WsCloseCode::Normal,
            CloseMode::Resumable => WsCloseCode::from(crate::core::RESUME_CLOSE_CODE),
        };
        let frame = CloseFrame {
            code,
            reason: "".into(),
        };
        let _ = tokio::time::timeout(Duration::from_secs(5), socket.close(Some(frame))).await;
        self.update_status();
        lock(&self.status).active = false;
        match mode {
            CloseMode::Resumable => self.core.session(),
            CloseMode::End => None,
        }
    }

    fn update_status(&self) {
        let mut status = lock(&self.status);
        status.latency = self.core.latency();
        status.session = self.core.session();
        status.active = self.core.is_active();
    }

    /// Sends an event to the consumer. Gives `false` when the consumer is gone.
    async fn emit(&self, event: ShardEvent) -> bool {
        if matches!(event, ShardEvent::Lifecycle(Lifecycle::HeartbeatAck { .. })) {
            self.update_status();
        }
        let message = ShardMessage {
            shard_id: self.config.shard[0],
            event,
        };
        self.events.send(message).await.is_ok()
    }
}
