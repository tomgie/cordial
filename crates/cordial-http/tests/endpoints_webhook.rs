//! Tests of the webhook endpoints with a fake transport.

use cordial_http::request::FileUpload;
use cordial_http::testing::{FakeResponse, FakeTransport, RecordedRequest};
use cordial_http::{Client, Error, ImageData, Token};
use cordial_model::embed::Embed;
use cordial_model::id::{ChannelId, ForumTagId, GuildId, MessageId, WebhookId};
use cordial_model::message::MessageFlags;
use cordial_model::webhook::WebhookType;
use serde_json::json;

const API: &str = "https://discord.com/api/v10";
const WEBHOOK: WebhookId = WebhookId::new_const(223704706495545344);
const TOKEN: &str = "3d89bb7572e0fb30d8128367b3b1b44fecd1726de135cbe28a41f8b2f777c372";
const CHANNEL: ChannelId = ChannelId::new_const(199737254929760256);

fn client(transport: &FakeTransport) -> Client {
    Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .build()
}

fn webhook_json() -> serde_json::Value {
    json!({
        "name": "test webhook",
        "type": 1,
        "channel_id": "199737254929760256",
        "token": TOKEN,
        "avatar": null,
        "guild_id": "199737254929760256",
        "id": "223704706495545344",
        "application_id": null,
        "user": {
            "username": "test",
            "discriminator": "7479",
            "id": "190320984123768832",
            "avatar": "b004ec1740a63ca06ae2e14c5cee11f3",
            "public_flags": 131328
        }
    })
}

fn message_json() -> serde_json::Value {
    json!({
        "id": "5678", "channel_id": "199737254929760256", "content": "Hello",
        "timestamp": "2026-01-01T00:00:00+00:00", "edited_timestamp": null,
        "tts": false, "mention_everyone": false, "mentions": [], "mention_roles": [],
        "attachments": [], "embeds": [], "pinned": false, "type": 0,
        "webhook_id": "223704706495545344",
        "author": {"id": "223704706495545344", "username": "test webhook", "discriminator": "0000", "avatar": null}
    })
}

fn only_request(transport: &FakeTransport) -> RecordedRequest {
    let requests = transport.requests();
    assert_eq!(requests.len(), 1);
    requests.into_iter().next().unwrap()
}

fn token_url(rest: &str) -> String {
    format!("{API}/webhooks/223704706495545344/{TOKEN}{rest}")
}

fn body_text(request: &RecordedRequest) -> String {
    String::from_utf8(request.body.to_vec()).unwrap()
}

#[tokio::test]
async fn creates_a_webhook() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &webhook_json()));

    let webhook = client(&transport)
        .create_webhook(CHANNEL, "test webhook")
        .avatar(ImageData::from_data_uri("data:image/png;base64,AAAA"))
        .reason("for alerts")
        .await
        .unwrap();

    assert_eq!(webhook.id, WEBHOOK);
    assert_eq!(webhook.kind, WebhookType::Incoming);
    assert_eq!(
        webhook.token.as_ref().map(cordial_model::Secret::expose),
        Some(TOKEN)
    );
    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        format!("{API}/channels/199737254929760256/webhooks")
    );
    assert_eq!(request.header("authorization"), Some("Bot abc"));
    assert_eq!(request.header("x-audit-log-reason"), Some("for%20alerts"));
    assert_eq!(
        request.json(),
        json!({"name": "test webhook", "avatar": "data:image/png;base64,AAAA"})
    );
}

#[tokio::test]
async fn refuses_webhook_name_with_discord() {
    let transport = FakeTransport::new();
    let client = client(&transport);

    let reserved = client.create_webhook(CHANNEL, "My Discord hook").await;
    let empty = client.create_webhook(CHANNEL, "").await;
    let long = client.edit_webhook(WEBHOOK).name("a".repeat(81)).await;

    assert!(matches!(reserved, Err(Error::Validation(_))));
    assert!(matches!(empty, Err(Error::Validation(_))));
    assert!(matches!(long, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn gets_channel_and_guild_webhooks() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &json!([webhook_json()])));
    transport.push(FakeResponse::json(200, &json!([webhook_json()])));
    let client = client(&transport);

    let channel_webhooks = client.channel_webhooks(CHANNEL).await.unwrap();
    let guild_webhooks = client.guild_webhooks(GuildId::new_const(42)).await.unwrap();

    assert_eq!(channel_webhooks.len(), 1);
    assert_eq!(guild_webhooks[0].name.as_deref(), Some("test webhook"));
    let requests = transport.requests();
    assert_eq!(requests[0].method, "GET");
    assert_eq!(
        requests[0].url,
        format!("{API}/channels/199737254929760256/webhooks")
    );
    assert_eq!(requests[1].method, "GET");
    assert_eq!(requests[1].url, format!("{API}/guilds/42/webhooks"));
    assert_eq!(requests[0].header("authorization"), Some("Bot abc"));
    assert_eq!(requests[1].header("authorization"), Some("Bot abc"));
}

#[tokio::test]
async fn gets_a_webhook() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &webhook_json()));

    let webhook = client(&transport).webhook(WEBHOOK).await.unwrap();

    assert!(webhook.user.is_some());
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, format!("{API}/webhooks/223704706495545344"));
    assert_eq!(request.header("authorization"), Some("Bot abc"));
}

#[tokio::test]
async fn gets_a_webhook_with_token_without_authorization() {
    let transport = FakeTransport::new();
    let mut webhook = webhook_json();
    webhook.as_object_mut().unwrap().remove("user");
    transport.push(FakeResponse::json(200, &webhook));
    let tokenless = Client::builder().transport(transport.clone()).build();

    let webhook = tokenless.webhook_with_token(WEBHOOK, TOKEN).await.unwrap();

    assert!(webhook.user.is_none());
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, token_url(""));
    assert_eq!(request.header("authorization"), None);
}

#[tokio::test]
async fn modifies_a_webhook() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &webhook_json()));

    client(&transport)
        .edit_webhook(WEBHOOK)
        .name("renamed")
        .clear_avatar()
        .channel_id(ChannelId::new_const(5))
        .reason("move")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.url, format!("{API}/webhooks/223704706495545344"));
    assert_eq!(request.header("authorization"), Some("Bot abc"));
    assert_eq!(request.header("x-audit-log-reason"), Some("move"));
    assert_eq!(
        request.json(),
        json!({"name": "renamed", "avatar": null, "channel_id": "5"})
    );
}

#[tokio::test]
async fn modifies_a_webhook_with_token() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &webhook_json()));

    client(&transport)
        .edit_webhook_with_token(WEBHOOK, TOKEN)
        .avatar(ImageData::from_data_uri("data:image/png;base64,AAAA"))
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(request.url, token_url(""));
    assert_eq!(request.header("authorization"), None);
    assert_eq!(
        request.json(),
        json!({"avatar": "data:image/png;base64,AAAA"})
    );
}

#[tokio::test]
async fn deletes_a_webhook() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_webhook(WEBHOOK)
        .reason("unused")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.url, format!("{API}/webhooks/223704706495545344"));
    assert_eq!(request.header("authorization"), Some("Bot abc"));
    assert_eq!(request.header("x-audit-log-reason"), Some("unused"));
}

#[tokio::test]
async fn deletes_a_webhook_with_token() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_webhook_with_token(WEBHOOK, TOKEN)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.url, token_url(""));
    assert_eq!(request.header("authorization"), None);
}

#[tokio::test]
async fn executes_a_webhook_without_wait() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    let () = client(&transport)
        .execute_webhook(WEBHOOK, TOKEN)
        .content("Hello")
        .username("Captain Hook")
        .avatar_url("https://example.com/hook.png")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, token_url(""));
    assert_eq!(request.header("authorization"), None);
    assert_eq!(
        request.json(),
        json!({
            "content": "Hello",
            "username": "Captain Hook",
            "avatar_url": "https://example.com/hook.png"
        })
    );
}

#[tokio::test]
async fn executes_a_webhook_and_waits_for_the_message() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &message_json()));

    let message = client(&transport)
        .execute_webhook(WEBHOOK, TOKEN)
        .embed(Embed::new().title("Build passed"))
        .thread_id(ChannelId::new_const(77))
        .with_components(true)
        .wait()
        .flags(MessageFlags::SUPPRESS_NOTIFICATIONS)
        .await
        .unwrap();

    assert_eq!(message.webhook_id, Some(WEBHOOK));
    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(
        request.url,
        token_url("?wait=true&thread_id=77&with_components=true")
    );
    assert_eq!(request.header("authorization"), None);
    assert_eq!(
        request.json(),
        json!({"embeds": [{"title": "Build passed"}], "flags": 4096})
    );
}

#[tokio::test]
async fn executes_a_webhook_with_a_forum_thread_and_files() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .execute_webhook(WEBHOOK, TOKEN)
        .content("See the log")
        .thread_name("Build 42")
        .applied_tags(vec![ForumTagId::new_const(9)])
        .file(FileUpload::new("log.txt", &b"ok"[..]).content_type("text/plain"))
        .await
        .unwrap();

    let request = only_request(&transport);
    assert!(
        request
            .header("content-type")
            .unwrap()
            .starts_with("multipart/form-data; boundary=")
    );
    let body = body_text(&request);
    assert!(body.contains(r#""thread_name":"Build 42""#));
    assert!(body.contains(r#""applied_tags":["9"]"#));
    assert!(body.contains(r#""attachments":[{"id":0,"filename":"log.txt"}]"#));
    assert!(body.contains("name=\"files[0]\"; filename=\"log.txt\""));
}

#[tokio::test]
async fn refuses_empty_or_wrongly_flagged_webhook_message() {
    let transport = FakeTransport::new();
    let client = client(&transport);

    let empty = client.execute_webhook(WEBHOOK, TOKEN).await;
    let ephemeral = client
        .execute_webhook(WEBHOOK, TOKEN)
        .content("hi")
        .flags(MessageFlags::EPHEMERAL)
        .await;

    assert!(matches!(empty, Err(Error::Validation(_))));
    assert!(matches!(ephemeral, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn executes_a_slack_webhook() {
    let transport = FakeTransport::new();
    // Discord answers with a body that is not JSON.
    let mut response = FakeResponse::empty(200).header("content-type", "text/html");
    response.body = "ok".into();
    transport.push(response);

    client(&transport)
        .execute_slack_webhook(WEBHOOK, TOKEN, json!({"text": "Hello from Slack"}))
        .thread_id(ChannelId::new_const(77))
        .wait(true)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, token_url("/slack?thread_id=77&wait=true"));
    assert_eq!(request.header("authorization"), None);
    assert_eq!(request.json(), json!({"text": "Hello from Slack"}));
}

#[tokio::test]
async fn executes_a_github_webhook() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));
    let payload = json!({
        "ref": "refs/heads/main",
        "repository": {"name": "cordial", "full_name": "example/cordial"},
        "commits": []
    });

    client(&transport)
        .execute_github_webhook(WEBHOOK, TOKEN, payload.clone())
        .event("push")
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, token_url("/github"));
    assert_eq!(request.header("authorization"), None);
    assert_eq!(request.header("x-github-event"), Some("push"));
    assert_eq!(request.json(), payload);
}

#[tokio::test]
async fn gets_a_webhook_message_in_a_thread() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &message_json()));

    let message = client(&transport)
        .webhook_message(WEBHOOK, TOKEN, MessageId::new_const(5678))
        .thread_id(ChannelId::new_const(77))
        .await
        .unwrap();

    assert_eq!(message.content, "Hello");
    let request = only_request(&transport);
    assert_eq!(request.method, "GET");
    assert_eq!(request.url, token_url("/messages/5678?thread_id=77"));
    assert_eq!(request.header("authorization"), None);
}

#[tokio::test]
async fn edits_a_webhook_message() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(200, &message_json()));

    client(&transport)
        .edit_webhook_message(WEBHOOK, TOKEN, MessageId::new_const(5678))
        .content("Edited")
        .thread_id(ChannelId::new_const(77))
        .with_components(true)
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "PATCH");
    assert_eq!(
        request.url,
        token_url("/messages/5678?thread_id=77&with_components=true")
    );
    assert_eq!(request.header("authorization"), None);
    assert_eq!(request.json(), json!({"content": "Edited"}));
}

#[tokio::test]
async fn deletes_a_webhook_message() {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));

    client(&transport)
        .delete_webhook_message(WEBHOOK, TOKEN, MessageId::new_const(5678))
        .await
        .unwrap();

    let request = only_request(&transport);
    assert_eq!(request.method, "DELETE");
    assert_eq!(request.url, token_url("/messages/5678"));
    assert_eq!(request.header("authorization"), None);
}

#[test]
fn request_debug_hides_the_token() {
    let client = Client::builder().transport(FakeTransport::new()).build();

    let text = format!("{:?}", client.execute_webhook(WEBHOOK, TOKEN));

    assert!(!text.contains(TOKEN));
}
