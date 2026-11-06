//! Tests of the HTTP client with a fake transport.

use std::time::Duration;

use cordial_http::request::FileUpload;
use cordial_http::testing::{FakeResponse, FakeTransport};
use cordial_http::{Client, Error, JsonErrorCode, Token};
use cordial_model::embed::Embed;
use cordial_model::id::{ChannelId, MessageId};
use serde_json::json;
use tokio::time::Instant;

const CHANNEL: ChannelId = ChannelId::new_const(1234);

fn message_json() -> serde_json::Value {
    json!({
        "id": "5", "channel_id": "1234", "content": "Hello",
        "timestamp": "2026-01-01T00:00:00+00:00", "edited_timestamp": null,
        "tts": false, "mention_everyone": false, "mentions": [], "mention_roles": [],
        "attachments": [], "embeds": [], "pinned": false, "type": 0,
        "author": {"id": "9", "username": "bot", "discriminator": "0", "global_name": null, "avatar": null}
    })
}

fn client(transport: &FakeTransport) -> Client {
    Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .build()
}

#[tokio::test]
async fn sends_a_message() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &message_json()).rate_limit(5, 4, 5.0, "bucket"));

    let message = client(&transport)
        .create_message(CHANNEL)
        .content("Hello")
        .reply_to(MessageId::new_const(77))
        .await
        .unwrap();

    assert_eq!(message.content, "Hello");
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        "https://discord.com/api/v10/channels/1234/messages"
    );
    assert_eq!(request.header("authorization"), Some("Bot abc"));
    assert_eq!(request.header("content-type"), Some("application/json"));
    assert!(
        request
            .header("user-agent")
            .unwrap()
            .starts_with("DiscordBot (")
    );
    assert_eq!(
        request.json(),
        json!({"content": "Hello", "message_reference": {"message_id": "77"}})
    );
}

#[tokio::test]
async fn sends_files_as_multipart() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &message_json()));

    client(&transport)
        .create_message(CHANNEL)
        .embed(Embed::new().image("attachment://cat.png"))
        .file(FileUpload::new("cat.png", &b"PNG"[..]).content_type("image/png"))
        .await
        .unwrap();

    let request = &transport.requests()[0];
    assert!(
        request
            .header("content-type")
            .unwrap()
            .starts_with("multipart/form-data; boundary=")
    );
    let body = String::from_utf8(request.body.to_vec()).unwrap();
    assert!(body.contains(r#""attachments":[{"id":0,"filename":"cat.png"}]"#));
    assert!(body.contains("name=\"files[0]\"; filename=\"cat.png\""));
}

#[tokio::test]
async fn refuses_invalid_message_without_sending() {
    let transport = FakeTransport::new();

    let result = client(&transport)
        .create_message(CHANNEL)
        .content("a".repeat(2001))
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn validation_can_be_turned_off() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &message_json()));
    let client = Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .without_validation()
        .build();

    let result = client
        .create_message(CHANNEL)
        .content("a".repeat(2001))
        .await;

    assert!(result.is_ok());
}

#[tokio::test(start_paused = true)]
async fn retries_after_bucket_429() {
    let transport = FakeTransport::new();
    transport.push(
        FakeResponse::json(
            429,
            &json!({"message": "You are being rate limited.", "retry_after": 2.0, "global": false}),
        )
        .rate_limit(5, 0, 2.0, "bucket")
        .header("x-ratelimit-scope", "user")
        .header("retry-after", "2"),
    );
    transport.push(FakeResponse::json(200, &message_json()).rate_limit(5, 4, 5.0, "bucket"));
    let client = client(&transport);
    let start = Instant::now();

    let result = client.create_message(CHANNEL).content("Hello").await;

    assert!(result.is_ok());
    assert_eq!(transport.requests().len(), 2);
    assert!(start.elapsed() >= Duration::from_secs(2));
    assert_eq!(client.invalid_request_count(), 1);
}

#[tokio::test(start_paused = true)]
async fn shared_429_does_not_count_as_invalid() {
    let transport = FakeTransport::new();
    transport.push(
        FakeResponse::json(
            429,
            &json!({"message": "Shared", "retry_after": 1.0, "global": false}),
        )
        .header("x-ratelimit-scope", "shared"),
    );
    transport.push(FakeResponse::json(200, &message_json()));
    let client = client(&transport);

    client
        .message(CHANNEL, MessageId::new_const(5))
        .await
        .unwrap();

    assert_eq!(client.invalid_request_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn global_429_pauses_other_routes() {
    let transport = FakeTransport::new();
    transport.push(
        FakeResponse::json(
            429,
            &json!({"message": "Global", "retry_after": 3.0, "global": true}),
        )
        .header("x-ratelimit-global", "true")
        .header("x-ratelimit-scope", "global"),
    );
    transport.push(FakeResponse::json(200, &message_json()));
    let client = Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .max_rate_limit_retries(0)
        .build();

    let first = client.message(CHANNEL, MessageId::new_const(5)).await;
    assert!(matches!(
        first,
        Err(Error::RateLimited { global: true, .. })
    ));

    let start = Instant::now();
    client
        .message(ChannelId::new_const(99), MessageId::new_const(5))
        .await
        .unwrap();
    assert!(start.elapsed() >= Duration::from_secs(3));
}

#[tokio::test]
async fn status_401_stops_all_requests() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        401,
        &json!({"message": "401: Unauthorized", "code": 0}),
    ));
    let client = client(&transport);

    let first = client.current_user().await;
    let second = client.current_user().await;

    assert_eq!(first.unwrap_err().status(), Some(401));
    assert!(matches!(second, Err(Error::InvalidToken)));
    assert_eq!(transport.requests().len(), 1);
}

#[tokio::test(start_paused = true)]
async fn retries_server_errors_only_when_safe() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(502));
    transport.push(FakeResponse::json(200, &message_json()));
    transport.push(FakeResponse::empty(502));
    let client = client(&transport);

    let get = client.message(CHANNEL, MessageId::new_const(5)).await;
    let post = client.create_message(CHANNEL).content("x").await;

    assert!(get.is_ok());
    assert_eq!(post.unwrap_err().status(), Some(502));
    assert_eq!(transport.requests().len(), 3);
}

#[tokio::test]
async fn decodes_api_errors() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(
        403,
        &json!({"message": "Missing Permissions", "code": 50013}),
    ));

    let error = client(&transport)
        .create_message(CHANNEL)
        .content("x")
        .await
        .unwrap_err();

    assert_eq!(error.code(), Some(JsonErrorCode::MissingPermissions));
}

#[tokio::test]
async fn sends_audit_log_reason_encoded() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_message(CHANNEL, MessageId::new_const(5))
        .reason("spam & ads")
        .await
        .unwrap();

    let request = &transport.requests()[0];
    assert_eq!(request.method, "DELETE");
    assert_eq!(
        request.header("x-audit-log-reason"),
        Some("spam%20%26%20ads")
    );
}

#[tokio::test]
async fn query_parameters() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &json!([])));

    let messages = client(&transport)
        .messages(CHANNEL)
        .before(MessageId::new_const(10))
        .limit(25)
        .await
        .unwrap();

    assert!(messages.is_empty());
    assert_eq!(
        transport.requests()[0].url,
        "https://discord.com/api/v10/channels/1234/messages?before=10&limit=25"
    );
}
