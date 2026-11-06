//! The Tokio driver of a call. It owns the voice WebSocket, the UDP socket, and the media
//! tasks, and it runs the actions of the [`VoiceCore`].

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode as WsCloseCode;

use crate::audio::mixer::Mixer;
use crate::call::{CallConfig, CallState, VoiceEvent, lock};
use crate::core::{ConnectionInfo, CoreEvent, ReconnectKind, VoiceAction, VoiceCore, VoiceFatal};
use crate::crypto::{SecretKey, TransportCipher};
use crate::e2ee::E2eeSession;
use crate::media::{SendTask, receive_loop};
use crate::packet::{IpDiscovery, ip_discovery_request};
use crate::protocol::{EncryptionMode, SpeakingFlags, VoiceCloseCode};
use crate::receive::{Receiver, SsrcMap};

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// A request of the send task: send the speaking flags, then reply.
pub(crate) type SpeakingRequest = (SpeakingFlags, oneshot::Sender<()>);

/// The close code that the driver uses to reconnect and keep the session (not 1000).
const RECONNECT_CLOSE_CODE: u16 = 4900;
/// The time to wait for a new voice server after close code 4014 (for example a move).
const MOVE_WAIT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DISCOVERY_ATTEMPTS: u32 = 3;
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(2);

/// A command from a [`Call`](crate::Call) handle.
#[derive(Debug)]
pub(crate) enum DriverCommand {
    /// Connect with new connection data.
    Connect(ConnectionInfo),
    /// The voice server is being moved: disconnect and wait for new data.
    ServerLost,
    /// Stop the call.
    Stop,
}

/// The parts of a call that the driver shares with the handles.
pub(crate) struct DriverParts {
    pub(crate) commands: mpsc::UnboundedReceiver<DriverCommand>,
    pub(crate) mixer: Arc<Mutex<Mixer>>,
    pub(crate) events: broadcast::Sender<VoiceEvent>,
    pub(crate) state: Arc<watch::Sender<CallState>>,
    pub(crate) latency: Arc<Mutex<Option<Duration>>>,
}

/// What the connection loop does next.
enum Next {
    Reconnect(Duration),
    WaitServer(Option<Duration>),
    Stop,
}

/// The effect of a command.
enum Effect {
    None,
    Reconnect,
    WaitServer,
    Stop,
}

pub(crate) struct Driver {
    core: VoiceCore,
    config: CallConfig,
    parts: DriverParts,
    users: SsrcMap,
    speaking_sender: mpsc::Sender<SpeakingRequest>,
    speaking_receiver: mpsc::Receiver<SpeakingRequest>,
    udp: Option<Arc<UdpSocket>>,
    media: Vec<JoinHandle<()>>,
    failures: u32,
    fatal: Option<VoiceFatal>,
}

impl Drop for Driver {
    fn drop(&mut self) {
        self.stop_media();
    }
}

impl Driver {
    pub(crate) fn new(
        info: ConnectionInfo,
        config: CallConfig,
        e2ee: Box<dyn E2eeSession>,
        parts: DriverParts,
    ) -> Self {
        let (speaking_sender, speaking_receiver) = mpsc::channel(4);
        Self {
            core: VoiceCore::new(info, Arc::new(Mutex::new(e2ee))),
            config,
            parts,
            users: SsrcMap::default(),
            speaking_sender,
            speaking_receiver,
            udp: None,
            media: Vec::new(),
            failures: 0,
            fatal: None,
        }
    }

    pub(crate) async fn run(mut self) {
        let mut wait_server = None;
        loop {
            if let Some(limit) = wait_server.take() {
                self.set_state(CallState::WaitingForServer);
                if !self.wait_for_server(limit).await {
                    break;
                }
            }

            let resume = self.core.will_resume();
            self.set_state(if resume {
                CallState::Reconnecting
            } else {
                CallState::Connecting
            });
            self.emit(VoiceEvent::Connecting);

            let url = self.core.info().websocket_url();
            let socket = match tokio::time::timeout(
                CONNECT_TIMEOUT,
                tokio_tungstenite::connect_async(url.as_str()),
            )
            .await
            {
                Ok(Ok((socket, _))) => socket,
                Ok(Err(error)) => {
                    tracing::warn!(%error, "voice gateway connection failed");
                    if !self.wait_after_failure().await {
                        break;
                    }
                    continue;
                }
                Err(_) => {
                    tracing::warn!("voice gateway connection timed out");
                    if !self.wait_after_failure().await {
                        break;
                    }
                    continue;
                }
            };

            match self.run_connection(socket).await {
                Next::Reconnect(delay) => {
                    if !delay.is_zero() && matches!(self.pause(delay).await, Effect::Stop) {
                        break;
                    }
                }
                Next::WaitServer(limit) => wait_server = Some(limit),
                Next::Stop => break,
            }
        }

        self.stop_media();
        let error = self.fatal.take();
        self.set_state(CallState::Stopped(error.clone()));
        self.emit(VoiceEvent::Stopped(error));
    }

    /// Waits after a failed connection. Gives `false` when the call must stop.
    async fn wait_after_failure(&mut self) -> bool {
        self.failures = self.failures.saturating_add(1);
        let seconds = 2_u64.saturating_pow(self.failures.min(5)).min(30);
        !matches!(self.pause(Duration::from_secs(seconds)).await, Effect::Stop)
    }

    /// Sleeps and handles commands. Gives the effect of the command that ends the sleep.
    async fn pause(&mut self, duration: Duration) -> Effect {
        let sleep = tokio::time::sleep(duration);
        tokio::pin!(sleep);
        loop {
            tokio::select! {
                () = &mut sleep => return Effect::None,
                command = self.parts.commands.recv() => match self.apply(command, false) {
                    Effect::None => {}
                    effect => return effect,
                },
                request = self.speaking_receiver.recv() => reply(request),
            }
        }
    }

    /// Waits for new connection data. Gives `false` when the call must stop.
    async fn wait_for_server(&mut self, limit: Option<Duration>) -> bool {
        self.stop_media();
        let deadline = limit.map(|limit| Instant::now() + limit);
        loop {
            let sleep_until =
                deadline.unwrap_or_else(|| Instant::now() + Duration::from_secs(3600));
            tokio::select! {
                () = tokio::time::sleep_until(sleep_until) => {
                    if deadline.is_some() {
                        return false;
                    }
                }
                command = self.parts.commands.recv() => match self.apply(command, true) {
                    Effect::Reconnect => return true,
                    Effect::Stop => return false,
                    Effect::None | Effect::WaitServer => {}
                },
                request = self.speaking_receiver.recv() => reply(request),
            }
        }
    }

    /// Applies a command. With `force`, new connection data always starts a new session.
    fn apply(&mut self, command: Option<DriverCommand>, force: bool) -> Effect {
        match command {
            None | Some(DriverCommand::Stop) => Effect::Stop,
            Some(DriverCommand::Connect(info)) => {
                if !force && !info.needs_new_session(self.core.info()) && !self.core.is_stopped() {
                    return Effect::None;
                }
                self.stop_media();
                self.fatal = None;
                self.failures = 0;
                self.core.set_info(info);
                Effect::Reconnect
            }
            Some(DriverCommand::ServerLost) => {
                self.stop_media();
                Effect::WaitServer
            }
        }
    }

    async fn run_connection(&mut self, mut socket: Socket) -> Next {
        self.core.connected(std::time::Instant::now());

        loop {
            if let Some(next) = self.do_actions(&mut socket).await {
                return next;
            }

            let deadline = self.core.next_deadline().map_or_else(
                || Instant::now() + Duration::from_secs(3600),
                Instant::from_std,
            );

            tokio::select! {
                frame = socket.next() => self.on_frame(frame),
                () = tokio::time::sleep_until(deadline) => {
                    self.core.tick(std::time::Instant::now());
                }
                command = self.parts.commands.recv() => match self.apply(command, false) {
                    Effect::None => {}
                    Effect::Reconnect => {
                        close(&mut socket, RECONNECT_CLOSE_CODE).await;
                        return Next::Reconnect(Duration::ZERO);
                    }
                    Effect::WaitServer => {
                        close(&mut socket, 1000).await;
                        return Next::WaitServer(None);
                    }
                    Effect::Stop => {
                        close(&mut socket, 1000).await;
                        return Next::Stop;
                    }
                },
                request = self.speaking_receiver.recv() => {
                    if let Some((flags, reply)) = request {
                        self.core.set_speaking(flags);
                        let next = self.do_actions(&mut socket).await;
                        let _ = reply.send(());
                        if let Some(next) = next {
                            return next;
                        }
                    }
                }
            }
        }
    }

    fn on_frame(&mut self, frame: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>) {
        let now = std::time::Instant::now();
        match frame {
            Some(Ok(Message::Text(text))) => {
                if let Err(error) = self.core.received_text(text.as_str(), now) {
                    tracing::warn!(%error, "the voice gateway sent a payload that is not valid");
                }
            }
            Some(Ok(Message::Binary(bytes))) => {
                if let Err(error) = self.core.received_binary(&bytes, now) {
                    tracing::warn!(%error, "the voice gateway sent a payload that is not valid");
                }
            }
            Some(Ok(Message::Close(frame))) => {
                let code = frame.map(|frame| u16::from(frame.code));
                tracing::info!(?code, "the voice gateway closed the connection");
                self.emit(VoiceEvent::Disconnected { code });
                self.core.closed(code, now);
            }
            Some(Ok(_)) => {}
            Some(Err(error)) => {
                tracing::warn!(%error, "voice gateway connection error");
                self.emit(VoiceEvent::Disconnected { code: None });
                self.core.closed(None, now);
            }
            None => {
                self.emit(VoiceEvent::Disconnected { code: None });
                self.core.closed(None, now);
            }
        }
    }

    /// Runs the queued actions of the core. Gives `Some` when the connection must end.
    async fn do_actions(&mut self, socket: &mut Socket) -> Option<Next> {
        while let Some(action) = self.core.poll_action() {
            match action {
                VoiceAction::SendText(text) => {
                    if let Err(error) = socket.send(Message::Text(text.into())).await {
                        tracing::warn!(%error, "cannot send to the voice gateway");
                        self.core.closed(None, std::time::Instant::now());
                    }
                }
                VoiceAction::SendBinary(bytes) => {
                    if let Err(error) = socket.send(Message::Binary(bytes.into())).await {
                        tracing::warn!(%error, "cannot send to the voice gateway");
                        self.core.closed(None, std::time::Instant::now());
                    }
                }
                VoiceAction::ConnectUdp { ip, port, ssrc } => {
                    self.stop_media();
                    self.udp = None;
                    match discover(&ip, port, ssrc).await {
                        Ok((udp, address, external_port)) => {
                            self.udp = Some(udp);
                            self.core.ip_discovered(
                                address,
                                external_port,
                                std::time::Instant::now(),
                            );
                        }
                        Err(error) => {
                            tracing::warn!(%error, "voice IP discovery failed");
                            self.core.ip_discovery_failed(std::time::Instant::now());
                        }
                    }
                }
                VoiceAction::SessionReady {
                    mode,
                    secret_key,
                    ssrc,
                    dave_protocol_version,
                } => {
                    self.start_media(mode, &secret_key, ssrc);
                    self.failures = 0;
                    self.fatal = None;
                    self.set_state(CallState::Ready);
                    self.emit(VoiceEvent::Ready {
                        ssrc,
                        mode,
                        dave_protocol_version,
                    });
                }
                VoiceAction::Event(event) => self.on_core_event(event),
                VoiceAction::Reconnect { kind, delay } => {
                    if kind == ReconnectKind::NewSession {
                        self.stop_media();
                    }
                    close(socket, RECONNECT_CLOSE_CODE).await;
                    return Some(Next::Reconnect(delay));
                }
                VoiceAction::Fatal(error) => {
                    tracing::warn!(%error, "the voice connection stops");
                    close(socket, 1000).await;
                    self.stop_media();
                    let moved = error == VoiceFatal::Closed(VoiceCloseCode::Disconnected);
                    self.fatal = Some(error);
                    if moved {
                        return Some(Next::WaitServer(Some(MOVE_WAIT)));
                    }
                    return Some(Next::Stop);
                }
            }
        }
        None
    }

    fn on_core_event(&mut self, event: CoreEvent) {
        match event {
            CoreEvent::Resumed => {
                self.set_state(CallState::Ready);
                self.emit(VoiceEvent::Resumed);
            }
            CoreEvent::HeartbeatAck { latency } => {
                *lock(&self.parts.latency) = Some(latency);
            }
            CoreEvent::UserSsrc { user_id, ssrc } => {
                lock(&self.users).insert(ssrc, user_id);
            }
            CoreEvent::Speaking {
                user_id,
                ssrc,
                flags,
            } => self.emit(VoiceEvent::Speaking {
                user_id,
                ssrc,
                flags,
            }),
            CoreEvent::ClientsConnect(user_ids) => self.emit(VoiceEvent::ClientsConnect(user_ids)),
            CoreEvent::ClientDisconnect(user_id) => {
                lock(&self.users).retain(|_, user| *user != user_id);
                self.emit(VoiceEvent::ClientDisconnect(user_id));
            }
            CoreEvent::E2eeError(error) => self.emit(VoiceEvent::E2eeError(error)),
            CoreEvent::Zombie => tracing::warn!("the voice connection is not alive"),
            _ => {}
        }
    }

    fn start_media(&mut self, mode: EncryptionMode, key: &SecretKey, ssrc: u32) {
        self.stop_media();
        let Some(udp) = self.udp.clone() else {
            tracing::warn!("the session is ready without a UDP socket");
            return;
        };
        let cipher = TransportCipher::new(mode, key);
        let e2ee = Arc::clone(self.core.e2ee());

        let sender = SendTask {
            socket: Arc::clone(&udp),
            cipher: cipher.clone(),
            ssrc,
            mixer: Arc::clone(&self.parts.mixer),
            e2ee: Arc::clone(&e2ee),
            speaking: self.speaking_sender.clone(),
            events: self.parts.events.clone(),
            flags: self.config.speaking,
            bitrate: self.config.bitrate,
        };
        self.media.push(tokio::spawn(sender.run()));

        if self.config.receive {
            let receiver = Receiver::new(
                cipher,
                e2ee,
                Arc::clone(&self.users),
                ssrc,
                self.config.jitter_depth,
                self.config.decode,
            );
            self.media.push(tokio::spawn(receive_loop(
                udp,
                receiver,
                self.parts.events.clone(),
            )));
        }
    }

    fn stop_media(&mut self) {
        for task in self.media.drain(..) {
            task.abort();
        }
    }

    fn set_state(&self, state: CallState) {
        self.parts.state.send_replace(state);
    }

    fn emit(&self, event: VoiceEvent) {
        let _ = self.parts.events.send(event);
    }
}

fn reply(request: Option<SpeakingRequest>) {
    if let Some((_, reply)) = request {
        let _ = reply.send(());
    }
}

async fn close(socket: &mut Socket, code: u16) {
    let frame = CloseFrame {
        code: WsCloseCode::from(code),
        reason: "".into(),
    };
    let _ = tokio::time::timeout(Duration::from_secs(2), socket.close(Some(frame))).await;
}

/// Opens a UDP socket to the voice server and finds the external address.
async fn discover(
    ip: &str,
    port: u16,
    ssrc: u32,
) -> std::io::Result<(Arc<UdpSocket>, String, u16)> {
    let bind = if ip.contains(':') {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };
    let socket = UdpSocket::bind(bind).await?;
    socket.connect((ip, port)).await?;
    let request = ip_discovery_request(ssrc);
    let mut buffer = [0_u8; 512];

    for _ in 0..DISCOVERY_ATTEMPTS {
        socket.send(&request).await?;
        let deadline = Instant::now() + DISCOVERY_TIMEOUT;
        loop {
            match tokio::time::timeout_at(deadline, socket.recv(&mut buffer)).await {
                Err(_) => break,
                // On Windows an ICMP error gives an error on the next receive. Try again.
                Ok(Err(error)) => {
                    tracing::debug!(%error, "IP discovery receive error");
                    break;
                }
                Ok(Ok(length)) => {
                    if let Ok(response) = IpDiscovery::decode(&buffer[..length]) {
                        return Ok((Arc::new(socket), response.address, response.port));
                    }
                }
            }
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "the voice server did not answer IP discovery",
    ))
}
