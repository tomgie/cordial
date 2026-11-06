//! Tests of the shard driver against a local mock gateway.

use std::time::Duration;

use cordial_gateway::{
    Compression, FatalError, Lifecycle, Shard, ShardConfig, ShardEvent, ShardMessage,
};
use cordial_http::Token;
use cordial_model::gateway::{Event, Intents};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

type ServerSocket = tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>;

async fn listen() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    (listener, url)
}

async fn accept(listener: &TcpListener) -> ServerSocket {
    let (stream, _) = listener.accept().await.unwrap();
    tokio_tungstenite::accept_async(stream).await.unwrap()
}

async fn send_json(socket: &mut ServerSocket, value: &Value) {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .unwrap();
}

/// Reads frames until a payload with the opcode arrives (heartbeats are skipped).
async fn expect_op(socket: &mut ServerSocket, op: u64) -> Value {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .expect("no payload within 5 seconds")
            .expect("the client closed")
            .unwrap();
        if let Message::Text(text) = message {
            let value: Value = serde_json::from_str(text.as_str()).unwrap();
            if value["op"] == op {
                return value;
            }
        }
    }
}

fn ready(url: &str) -> Value {
    json!({"op": 0, "s": 1, "t": "READY", "d": {
        "v": 10,
        "user": {"id": "1", "username": "bot", "discriminator": "0", "avatar": null, "bot": true},
        "guilds": [{"id": "100", "unavailable": true}],
        "session_id": "session-1",
        "resume_gateway_url": url,
        "application": {"id": "1", "flags": 0}
    }})
}

async fn next_matching(
    events: &mut mpsc::Receiver<ShardMessage>,
    check: impl Fn(&ShardEvent) -> bool,
) -> ShardEvent {
    loop {
        let message = tokio::time::timeout(Duration::from_secs(5), events.recv())
            .await
            .expect("no matching event within 5 seconds")
            .expect("the shard stopped");
        if check(&message.event) {
            return message.event;
        }
    }
}

fn config(url: &str) -> ShardConfig {
    ShardConfig::new(
        Token::bot("TOKEN"),
        Intents::GUILDS | Intents::GUILD_MESSAGES,
    )
    .gateway_url(url)
    .compression(Compression::None)
}

#[tokio::test]
async fn identifies_receives_events_and_resumes() {
    let (listener, url) = listen().await;
    let (shard, mut events) = Shard::start(config(&url));

    // First connection: Hello, Identify, READY, and one event.
    let mut socket = accept(&listener).await;
    send_json(
        &mut socket,
        &json!({"op": 10, "d": {"heartbeat_interval": 45000}}),
    )
    .await;
    let identify = expect_op(&mut socket, 2).await;
    assert_eq!(identify["d"]["token"], "TOKEN");
    assert_eq!(identify["d"]["intents"], 513);
    send_json(&mut socket, &ready(&url)).await;
    send_json(
        &mut socket,
        &json!({"op": 0, "s": 2, "t": "MESSAGE_DELETE", "d": {"id": "5", "channel_id": "6"}}),
    )
    .await;

    next_matching(&mut events, |event| {
        matches!(event, ShardEvent::Lifecycle(Lifecycle::Ready { .. }))
    })
    .await;
    let delete = next_matching(&mut events, |event| {
        matches!(event, ShardEvent::Dispatch(event) if matches!(**event, Event::MessageDelete(_)))
    })
    .await;
    assert!(matches!(delete, ShardEvent::Dispatch(_)));
    assert!(shard.is_active());

    // The server drops the connection with a code that permits a resume.
    socket
        .close(Some(CloseFrame {
            code: CloseCode::Library(4000),
            reason: "".into(),
        }))
        .await
        .unwrap();

    // Second connection: the client resumes with the last sequence.
    let mut socket = accept(&listener).await;
    send_json(
        &mut socket,
        &json!({"op": 10, "d": {"heartbeat_interval": 45000}}),
    )
    .await;
    let resume = expect_op(&mut socket, 6).await;
    assert_eq!(resume["d"]["session_id"], "session-1");
    assert_eq!(resume["d"]["seq"], 2);
    send_json(
        &mut socket,
        &json!({"op": 0, "s": 3, "t": "RESUMED", "d": null}),
    )
    .await;

    next_matching(&mut events, |event| {
        matches!(event, ShardEvent::Lifecycle(Lifecycle::Resumed))
    })
    .await;
    assert_eq!(shard.session().unwrap().sequence, 3);
}

#[tokio::test]
async fn sends_commands_after_ready() {
    let (listener, url) = listen().await;
    let (shard, mut events) = Shard::start(config(&url));
    let mut socket = accept(&listener).await;
    send_json(
        &mut socket,
        &json!({"op": 10, "d": {"heartbeat_interval": 45000}}),
    )
    .await;
    expect_op(&mut socket, 2).await;
    send_json(&mut socket, &ready(&url)).await;
    next_matching(&mut events, |event| {
        matches!(event, ShardEvent::Lifecycle(Lifecycle::Ready { .. }))
    })
    .await;

    let request = cordial_model::gateway::RequestGuildMembers::query(
        cordial_model::id::GuildId::new_const(100),
        "ab",
        10,
    );
    shard
        .send(cordial_gateway::Command::RequestGuildMembers(request))
        .await
        .unwrap();

    let payload = expect_op(&mut socket, 8).await;
    assert_eq!(payload["d"]["query"], "ab");
}

#[tokio::test]
async fn stops_on_fatal_close_code() {
    let (listener, url) = listen().await;
    let (shard, mut events) = Shard::start(config(&url));
    let mut socket = accept(&listener).await;
    send_json(
        &mut socket,
        &json!({"op": 10, "d": {"heartbeat_interval": 45000}}),
    )
    .await;
    expect_op(&mut socket, 2).await;

    socket
        .close(Some(CloseFrame {
            code: CloseCode::Library(4004),
            reason: "Authentication failed.".into(),
        }))
        .await
        .unwrap();

    let fatal = next_matching(&mut events, |event| matches!(event, ShardEvent::Fatal(_))).await;
    assert!(matches!(
        fatal,
        ShardEvent::Fatal(FatalError::Closed { code: 4004 })
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(shard.is_stopped());
}

#[tokio::test]
async fn reconnect_opcode_makes_a_new_connection() {
    let (listener, url) = listen().await;
    let (_shard, mut events) = Shard::start(config(&url));
    let mut socket = accept(&listener).await;
    send_json(
        &mut socket,
        &json!({"op": 10, "d": {"heartbeat_interval": 45000}}),
    )
    .await;
    expect_op(&mut socket, 2).await;
    send_json(&mut socket, &ready(&url)).await;
    next_matching(&mut events, |event| {
        matches!(event, ShardEvent::Lifecycle(Lifecycle::Ready { .. }))
    })
    .await;

    send_json(&mut socket, &json!({"op": 7, "d": null})).await;

    let mut socket = accept(&listener).await;
    send_json(
        &mut socket,
        &json!({"op": 10, "d": {"heartbeat_interval": 45000}}),
    )
    .await;
    let resume = expect_op(&mut socket, 6).await;
    assert_eq!(resume["d"]["session_id"], "session-1");
}

#[tokio::test]
async fn collects_member_chunks_by_nonce() {
    let (listener, url) = listen().await;
    let (shard, mut events) = Shard::start(config(&url));
    let mut socket = accept(&listener).await;
    send_json(
        &mut socket,
        &json!({"op": 10, "d": {"heartbeat_interval": 45000}}),
    )
    .await;
    expect_op(&mut socket, 2).await;
    send_json(&mut socket, &ready(&url)).await;
    next_matching(&mut events, |event| {
        matches!(event, ShardEvent::Lifecycle(Lifecycle::Ready { .. }))
    })
    .await;

    let guild = cordial_model::id::GuildId::new_const(100);
    let request = cordial_model::gateway::RequestGuildMembers::users(
        guild,
        vec![
            cordial_model::id::UserId::new_const(7),
            cordial_model::id::UserId::new_const(8),
        ],
    );
    let chunks = shard.request_members(request).await.unwrap();

    let payload = expect_op(&mut socket, 8).await;
    let nonce = payload["d"]["nonce"].as_str().unwrap().to_owned();
    let member = |id: &str| json!({"user": {"id": id, "username": "u", "discriminator": "0", "avatar": null}, "roles": [], "joined_at": "2026-01-01T00:00:00+00:00", "deaf": false, "mute": false, "flags": 0});
    for (index, id) in ["7", "8"].iter().enumerate() {
        send_json(&mut socket, &json!({"op": 0, "s": 2 + index, "t": "GUILD_MEMBERS_CHUNK", "d": {
            "guild_id": "100", "members": [member(id)], "chunk_index": index, "chunk_count": 2, "nonce": nonce
        }}))
        .await;
    }

    let (members, not_found) = tokio::time::timeout(Duration::from_secs(5), chunks.collect())
        .await
        .unwrap();
    assert_eq!(members.len(), 2);
    assert!(not_found.is_empty());
}
