//! Tests of the Tokio driver against a local mock voice server: a WebSocket server and a UDP
//! socket on 127.0.0.1.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{AeadInPlace, KeyInit};
use cordial_model::gateway::UpdateVoiceState;
use cordial_model::gateway::event::Event;
use cordial_model::id::{ChannelId, GuildId, UserId};
use cordial_voice::audio::ogg::OggOpusWriter;
use cordial_voice::packet::IpDiscovery;
use cordial_voice::{
    Call, CallConfig, CallState, ConnectionInfo, GatewayFuture, NoE2ee, OggOpusSource, VoiceEvent,
    VoiceGateway, VoiceManager,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc;
use tokio::time::{Instant, timeout};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

const GUILD: GuildId = GuildId::new_const(41_771_983_423_143_937);
const CHANNEL: ChannelId = ChannelId::new_const(127_121_515_262_115_840);
const USER: UserId = UserId::new_const(104_694_319_306_248_192);
const SSRC: u32 = 42;
const KEY: [u8; 32] = [7; 32];
const WAIT: Duration = Duration::from_secs(5);

type Server = WebSocketStream<TcpStream>;

struct MockServer {
    listener: TcpListener,
    udp: Arc<UdpSocket>,
}

impl MockServer {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind TCP");
        let udp = UdpSocket::bind("127.0.0.1:0").await.expect("bind UDP");
        Self {
            listener,
            udp: Arc::new(udp),
        }
    }

    fn endpoint(&self) -> String {
        let port = self.listener.local_addr().expect("address").port();
        format!("ws://127.0.0.1:{port}")
    }

    async fn accept(&self) -> Server {
        let (stream, _) = timeout(WAIT, self.listener.accept())
            .await
            .expect("a client connects")
            .expect("accept");
        tokio_tungstenite::accept_async(stream)
            .await
            .expect("WebSocket handshake")
    }

    /// Runs Hello, Identify, Ready, IP discovery, Select Protocol, and Session Description.
    /// Gives the UDP address of the client.
    async fn handshake(&self, server: &mut Server) -> SocketAddr {
        send(
            server,
            json!({"op": 8, "d": {"heartbeat_interval": 5000.0}}),
        )
        .await;

        let identify = receive(server).await;
        assert_eq!(identify["op"], 0);
        assert_eq!(
            identify["d"],
            json!({
                "server_id": GUILD.to_string(),
                "user_id": USER.to_string(),
                "session_id": "session",
                "token": "token",
                "max_dave_protocol_version": 0
            })
        );

        let udp_port = self.udp.local_addr().expect("address").port();
        send(
            server,
            json!({"op": 2, "seq": 1, "d": {
                "ssrc": SSRC,
                "ip": "127.0.0.1",
                "port": udp_port,
                "modes": ["aead_xchacha20_poly1305_rtpsize", "xsalsa20_poly1305"],
                "heartbeat_interval": 1
            }}),
        )
        .await;

        let mut buffer = [0_u8; 128];
        let (length, client) = timeout(WAIT, self.udp.recv_from(&mut buffer))
            .await
            .expect("IP discovery arrives")
            .expect("receive");
        assert_eq!(length, 74);
        assert_eq!(&buffer[..8], &[0, 1, 0, 70, 0, 0, 0, 42]);
        let response = IpDiscovery::new(SSRC, client.ip().to_string(), client.port())
            .encode()
            .expect("encode");
        self.udp.send_to(&response, client).await.expect("send");

        let select = receive(server).await;
        assert_eq!(
            select,
            json!({"op": 1, "d": {"protocol": "udp", "data": {
                "address": "127.0.0.1",
                "port": client.port(),
                "mode": "aead_xchacha20_poly1305_rtpsize"
            }}})
        );

        send(
            server,
            json!({"op": 4, "seq": 2, "d": {
                "mode": "aead_xchacha20_poly1305_rtpsize",
                "secret_key": KEY,
                "dave_protocol_version": 0
            }}),
        )
        .await;
        client
    }
}

async fn send(server: &mut Server, value: Value) {
    server
        .send(Message::Text(value.to_string().into()))
        .await
        .expect("send");
}

/// Receives the next JSON message. Answers heartbeats on the way.
async fn receive(server: &mut Server) -> Value {
    loop {
        let message = timeout(WAIT, server.next())
            .await
            .expect("a message arrives")
            .expect("the connection is open")
            .expect("valid frame");
        let Message::Text(text) = message else {
            continue;
        };
        let value: Value = serde_json::from_str(text.as_str()).expect("JSON");
        if value["op"] == 3 {
            let nonce = value["d"]["t"].clone();
            send(server, json!({"op": 6, "d": {"t": nonce}})).await;
            continue;
        }
        return value;
    }
}

/// Reads WebSocket messages in the background and gives them with their arrival time.
fn record(mut server: Server) -> mpsc::UnboundedReceiver<(Instant, Value)> {
    let (sender, receiver) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        while let Some(Ok(message)) = server.next().await {
            if let Message::Text(text) = message {
                let value: Value = serde_json::from_str(text.as_str()).expect("JSON");
                if sender.send((Instant::now(), value)).is_err() {
                    return;
                }
            }
        }
    });
    receiver
}

/// A received and decrypted RTP packet.
#[derive(Debug)]
struct Received {
    at: Instant,
    sequence: u16,
    timestamp: u32,
    ssrc: u32,
    payload: Vec<u8>,
}

/// Decrypts a packet with the cipher crate: header as AAD, the nonce from the last 4 bytes.
fn decrypt(packet: &[u8]) -> Received {
    assert_eq!(packet[0], 0x80);
    assert_eq!(packet[1], 0x78);
    let (header, rest) = packet.split_at(12);
    let (sealed, nonce_bytes) = rest.split_at(rest.len() - 4);
    let (ciphertext, tag) = sealed.split_at(sealed.len() - 16);
    let mut nonce = [0_u8; 24];
    nonce[..4].copy_from_slice(nonce_bytes);
    let mut payload = ciphertext.to_vec();
    XChaCha20Poly1305::new((&KEY).into())
        .decrypt_in_place_detached((&nonce).into(), header, &mut payload, tag.into())
        .expect("the packet decrypts");
    Received {
        at: Instant::now(),
        sequence: u16::from_be_bytes([header[2], header[3]]),
        timestamp: u32::from_be_bytes([header[4], header[5], header[6], header[7]]),
        ssrc: u32::from_be_bytes([header[8], header[9], header[10], header[11]]),
        payload,
    }
}

async fn receive_packets(udp: &UdpSocket, count: usize) -> Vec<Received> {
    let mut packets = Vec::new();
    let mut buffer = [0_u8; 2048];
    while packets.len() < count {
        let (length, _) = timeout(WAIT, udp.recv_from(&mut buffer))
            .await
            .expect("a packet arrives")
            .expect("receive");
        packets.push(decrypt(&buffer[..length]));
    }
    packets
}

fn ogg_file(packets: &[Vec<u8>]) -> Vec<u8> {
    let mut writer = OggOpusWriter::new(Vec::new(), 2, 99).expect("write");
    for packet in packets {
        writer.write_packet(packet).expect("write");
    }
    writer.finish().expect("write")
}

fn info(endpoint: String) -> ConnectionInfo {
    ConnectionInfo::new(
        GUILD,
        CHANNEL,
        USER,
        "session".to_owned(),
        "token".to_owned(),
        endpoint,
    )
}

fn check_sequence(packets: &[Received]) {
    for pair in packets.windows(2) {
        assert_eq!(pair[1].sequence, pair[0].sequence.wrapping_add(1));
        assert_eq!(pair[1].timestamp, pair[0].timestamp.wrapping_add(960));
        assert_eq!(pair[1].ssrc, SSRC);
    }
}

#[tokio::test]
async fn ogg_passthrough_reaches_the_server() {
    let server = MockServer::start().await;
    let call = Call::connect(
        info(server.endpoint()),
        CallConfig::default(),
        Box::new(NoE2ee::default()),
    );
    let mut events = call.subscribe();

    let mut socket = server.accept().await;
    server.handshake(&mut socket).await;
    call.wait_ready(WAIT).await.expect("the call becomes ready");
    assert_eq!(call.state(), CallState::Ready);
    let mut messages = record(socket);

    let audio: Vec<Vec<u8>> = (0..6_u8)
        .map(|index| vec![0xFC, index, 0xAA, index])
        .collect();
    let source = OggOpusSource::from_bytes(ogg_file(&audio)).expect("valid Ogg");
    let track = call.play(source);

    let packets = receive_packets(&server.udp, audio.len() + 5).await;
    check_sequence(&packets);
    for (packet, expected) in packets.iter().zip(&audio) {
        assert_eq!(&packet.payload, expected);
    }
    for packet in &packets[audio.len()..] {
        assert_eq!(packet.payload, vec![0xF8, 0xFF, 0xFE]);
    }

    // Speaking is the first message after the handshake, and Speaking 0 follows the silence.
    // The send task waits for the WebSocket write of Speaking before the first packet. The
    // arrival times here cannot show that order, because one runtime reads both sockets.
    let (_, speaking) = timeout(WAIT, messages.recv())
        .await
        .expect("Speaking arrives")
        .expect("open");
    assert_eq!(
        speaking,
        json!({"op": 5, "d": {"speaking": 1, "delay": 0, "ssrc": SSRC}})
    );
    let (_, silent) = timeout(WAIT, messages.recv())
        .await
        .expect("Speaking 0 arrives")
        .expect("open");
    assert_eq!(silent["d"]["speaking"], 0);

    // The packets keep the 20 ms clock.
    let span = packets[audio.len() + 4].at - packets[0].at;
    assert!(span >= Duration::from_millis(150), "span {span:?}");
    assert_eq!(track.state(), cordial_voice::PlayState::Ended);

    let mut saw_ready = false;
    let mut saw_end = false;
    while let Ok(event) = events.try_recv() {
        match event {
            VoiceEvent::Ready {
                ssrc,
                dave_protocol_version,
                ..
            } => {
                assert_eq!(ssrc, SSRC);
                assert_eq!(dave_protocol_version, 0);
                saw_ready = true;
            }
            VoiceEvent::Track(cordial_voice::TrackEvent::Ended { track: id, .. }) => {
                assert_eq!(id, track.id());
                saw_end = true;
            }
            _ => {}
        }
    }
    assert!(saw_ready);
    assert!(saw_end);

    call.disconnect();
    timeout(WAIT, async {
        while !matches!(call.state(), CallState::Stopped(_)) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the call stops");
}

#[cfg(feature = "opus")]
#[tokio::test]
async fn pcm_is_encoded() {
    let server = MockServer::start().await;
    let call = Call::connect(
        info(server.endpoint()),
        CallConfig::default(),
        Box::new(NoE2ee::default()),
    );
    let mut socket = server.accept().await;
    server.handshake(&mut socket).await;
    call.wait_ready(WAIT).await.expect("ready");
    let _messages = record(socket);

    let frames = 5;
    let samples: Vec<i16> = (0..cordial_voice::audio::FRAME_LEN * frames)
        .map(|index| if (index / 2) % 100 < 50 { 8000 } else { -8000 })
        .collect();
    call.play(cordial_voice::PcmSource::from_i16(samples));

    let packets = receive_packets(&server.udp, frames + 5).await;
    check_sequence(&packets);
    let mut decoder = cordial_voice::audio::codec::OpusDecoder::new().expect("decoder");
    for packet in &packets[..frames] {
        let pcm = decoder.decode(&packet.payload).expect("valid Opus");
        assert_eq!(pcm.len(), cordial_voice::audio::FRAME_LEN);
    }
    assert_eq!(packets[frames].payload, vec![0xF8, 0xFF, 0xFE]);
    call.disconnect();
}

#[tokio::test]
async fn resume_after_server_crash() {
    let server = MockServer::start().await;
    let call = Call::connect(
        info(server.endpoint()),
        CallConfig::default(),
        Box::new(NoE2ee::default()),
    );
    let mut events = call.subscribe();
    let mut socket = server.accept().await;
    server.handshake(&mut socket).await;
    call.wait_ready(WAIT).await.expect("ready");

    let frame = CloseFrame {
        code: CloseCode::from(4015),
        reason: "".into(),
    };
    socket.close(Some(frame)).await.expect("close");

    let mut socket = server.accept().await;
    send(
        &mut socket,
        json!({"op": 8, "d": {"heartbeat_interval": 5000.0}}),
    )
    .await;
    let resume = receive(&mut socket).await;
    assert_eq!(
        resume,
        json!({"op": 7, "d": {
            "server_id": GUILD.to_string(),
            "session_id": "session",
            "token": "token",
            "seq_ack": 2
        }})
    );
    send(&mut socket, json!({"op": 9, "d": null})).await;

    timeout(WAIT, async {
        loop {
            if let Ok(VoiceEvent::Resumed) = events.recv().await {
                return;
            }
        }
    })
    .await
    .expect("the call resumes");
    assert_eq!(call.state(), CallState::Ready);

    // The media of the old session continues after the resume.
    let _messages = record(socket);
    call.play(OggOpusSource::from_bytes(ogg_file(&[vec![0xFC, 1]])).expect("valid Ogg"));
    let packets = receive_packets(&server.udp, 1).await;
    assert_eq!(packets[0].payload, vec![0xFC, 1]);
    call.disconnect();
}

#[tokio::test]
async fn fatal_close_code_stops_the_call() {
    let server = MockServer::start().await;
    let call = Call::connect(
        info(server.endpoint()),
        CallConfig::default(),
        Box::new(NoE2ee::default()),
    );
    let mut socket = server.accept().await;
    server.handshake(&mut socket).await;
    call.wait_ready(WAIT).await.expect("ready");

    let frame = CloseFrame {
        code: CloseCode::from(4022),
        reason: "".into(),
    };
    socket.close(Some(frame)).await.expect("close");
    let stopped = timeout(WAIT, async {
        loop {
            if let CallState::Stopped(error) = call.state() {
                return error;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the call stops");
    assert_eq!(
        stopped,
        Some(cordial_voice::core::VoiceFatal::Closed(
            cordial_voice::VoiceCloseCode::CallTerminated
        ))
    );
}

/// A main gateway that records Update Voice State.
struct FakeGateway {
    sent: mpsc::UnboundedSender<UpdateVoiceState>,
}

impl VoiceGateway for FakeGateway {
    fn update_voice_state(&self, update: UpdateVoiceState) -> GatewayFuture<'_> {
        let _ = self.sent.send(update);
        Box::pin(async { Ok(()) })
    }
}

fn event(name: &str, data: &Value) -> Event {
    let raw = serde_json::value::RawValue::from_string(data.to_string()).expect("raw JSON");
    Event::decode(name, &raw).expect("valid event")
}

#[tokio::test]
async fn manager_joins_and_leaves() {
    let server = MockServer::start().await;
    let (sent, mut commands) = mpsc::unbounded_channel();
    let manager = VoiceManager::new(FakeGateway { sent });
    manager.handle_event(&event(
        "VOICE_STATE_UPDATE",
        &json!({"guild_id": GUILD.to_string(), "channel_id": CHANNEL.to_string(),
            "user_id": USER.to_string(), "session_id": "session"}),
    ));
    assert_eq!(
        manager.join(GUILD, CHANNEL).await.err(),
        Some(cordial_voice::JoinError::UnknownUser)
    );
    manager.set_user_id(USER);

    let joiner = manager.clone();
    let join = tokio::spawn(async move { joiner.join(GUILD, CHANNEL).await });

    let update = timeout(WAIT, commands.recv())
        .await
        .expect("op 4 is sent")
        .expect("open");
    assert_eq!(update.guild_id, GUILD);
    assert_eq!(update.channel_id, Some(CHANNEL));

    // The server update comes first; a null endpoint means: wait for the next one.
    manager.handle_event(&event(
        "VOICE_SERVER_UPDATE",
        &json!({"token": "old", "guild_id": GUILD.to_string(), "endpoint": null}),
    ));
    manager.handle_event(&event(
        "VOICE_SERVER_UPDATE",
        &json!({"token": "token", "guild_id": GUILD.to_string(), "endpoint": server.endpoint()}),
    ));
    // Another user does not count.
    manager.handle_event(&event(
        "VOICE_STATE_UPDATE",
        &json!({"guild_id": GUILD.to_string(), "channel_id": CHANNEL.to_string(),
            "user_id": "5", "session_id": "other"}),
    ));
    manager.handle_event(&event(
        "VOICE_STATE_UPDATE",
        &json!({"guild_id": GUILD.to_string(), "channel_id": CHANNEL.to_string(),
            "user_id": USER.to_string(), "session_id": "session"}),
    ));

    let mut socket = server.accept().await;
    server.handshake(&mut socket).await;
    let call = timeout(WAIT, join)
        .await
        .expect("join ends")
        .expect("task")
        .expect("join succeeds");
    assert_eq!(call.guild_id(), GUILD);
    assert_eq!(call.channel_id(), CHANNEL);
    assert!(manager.call(GUILD).is_some());

    manager.leave(GUILD).await.expect("leave");
    let update = timeout(WAIT, commands.recv())
        .await
        .expect("op 4 is sent")
        .expect("open");
    assert_eq!(update.channel_id, None);
    assert!(manager.call(GUILD).is_none());
}

#[tokio::test]
async fn manager_moves_the_call() {
    let first_server = MockServer::start().await;
    let second_server = MockServer::start().await;
    let (sent, mut commands) = mpsc::unbounded_channel();
    let manager = VoiceManager::new(FakeGateway { sent });
    manager.set_user_id(USER);

    let joiner = manager.clone();
    let join = tokio::spawn(async move { joiner.join(GUILD, CHANNEL).await });
    commands.recv().await.expect("op 4");
    manager.handle_event(&event(
        "VOICE_STATE_UPDATE",
        &json!({"guild_id": GUILD.to_string(), "channel_id": CHANNEL.to_string(),
            "user_id": USER.to_string(), "session_id": "session"}),
    ));
    manager.handle_event(&event(
        "VOICE_SERVER_UPDATE",
        &json!({"token": "token", "guild_id": GUILD.to_string(), "endpoint": first_server.endpoint()}),
    ));
    let mut socket = first_server.accept().await;
    first_server.handshake(&mut socket).await;
    let call = join.await.expect("task").expect("join");

    // A move: a new channel and a new token on another server.
    let other_channel = ChannelId::new_const(555);
    let mover = manager.clone();
    let moving = tokio::spawn(async move { mover.join(GUILD, other_channel).await });
    let update = commands.recv().await.expect("op 4");
    assert_eq!(update.channel_id, Some(other_channel));
    manager.handle_event(&event(
        "VOICE_STATE_UPDATE",
        &json!({"guild_id": GUILD.to_string(), "channel_id": other_channel.to_string(),
            "user_id": USER.to_string(), "session_id": "session"}),
    ));
    manager.handle_event(&event(
        "VOICE_SERVER_UPDATE",
        &json!({"token": "token", "guild_id": GUILD.to_string(), "endpoint": second_server.endpoint()}),
    ));

    let mut socket = second_server.accept().await;
    second_server.handshake(&mut socket).await;
    let moved = timeout(WAIT, moving)
        .await
        .expect("move ends")
        .expect("task")
        .expect("move succeeds");
    assert_eq!(moved.channel_id(), other_channel);
    assert_eq!(call.channel_id(), other_channel);

    // The bot is disconnected from voice: the call stops.
    manager.handle_event(&event(
        "VOICE_STATE_UPDATE",
        &json!({"guild_id": GUILD.to_string(), "channel_id": null,
            "user_id": USER.to_string(), "session_id": "session"}),
    ));
    assert!(manager.call(GUILD).is_none());
    timeout(WAIT, async {
        while !matches!(call.state(), CallState::Stopped(_)) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the call stops");
}

#[tokio::test]
async fn received_audio_has_the_user() {
    let server = MockServer::start().await;
    let call = Call::connect(
        info(server.endpoint()),
        CallConfig::default().receive(true),
        Box::new(NoE2ee::default()),
    );
    let mut events = call.subscribe();
    let mut socket = server.accept().await;
    let client = server.handshake(&mut socket).await;
    call.wait_ready(WAIT).await.expect("ready");
    send(
        &mut socket,
        json!({"op": 5, "seq": 3, "d": {"user_id": "77", "ssrc": 500, "speaking": 1}}),
    )
    .await;
    let _messages = record(socket);

    // Wait for the Speaking event, so that the SSRC map has the user.
    timeout(WAIT, async {
        loop {
            if let Ok(VoiceEvent::Speaking { user_id, ssrc, .. }) = events.recv().await {
                assert_eq!(user_id, UserId::new_const(77));
                assert_eq!(ssrc, 500);
                return;
            }
        }
    })
    .await
    .expect("Speaking event");

    // Sequence 12 comes before 11, and 13 is lost.
    let mut cipher = cordial_voice::crypto::TransportCipher::new(
        cordial_voice::EncryptionMode::XChaCha20Poly1305RtpSize,
        &cordial_voice::crypto::SecretKey::new(KEY),
    );
    for sequence in [10_u16, 12, 11, 14, 15] {
        let header =
            cordial_voice::packet::RtpHeader::new(sequence, u32::from(sequence) * 960, 500)
                .encode();
        let packet = cipher
            .encrypt(&header, &[0xF8, 0xFF, 0xFE])
            .expect("encrypt");
        server.udp.send_to(&packet, client).await.expect("send");
    }

    let mut sequences = Vec::new();
    timeout(WAIT, async {
        while sequences.len() < 6 {
            if let Ok(VoiceEvent::Audio(data)) = events.recv().await {
                assert_eq!(data.user_id, Some(UserId::new_const(77)));
                assert_eq!(data.ssrc, 500);
                assert_eq!(data.pcm.is_some(), cfg!(feature = "opus"));
                sequences.push((data.sequence, data.is_lost()));
            }
        }
    })
    .await
    .expect("audio events");
    assert_eq!(
        sequences,
        vec![
            (10, false),
            (11, false),
            (12, false),
            (13, true),
            (14, false),
            (15, false)
        ]
    );
    call.disconnect();
}
