//! Tests of the endpoint with real signatures.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cordial_endpoint::{Endpoint, Handler, PublicKey, Reply};
use cordial_model::interaction::{Interaction, InteractionMessage, InteractionResponse};
use cordial_model::webhook_event::WebhookEventBody;
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};

struct Bot {
    events: Arc<AtomicUsize>,
}

impl Handler for Bot {
    async fn interaction(&self, interaction: Interaction) -> Reply {
        let name = interaction.command_data().map(|data| data.name.clone());
        if name.as_deref() == Some("defer") {
            return Reply::Acknowledged;
        }
        let mut message = InteractionMessage::default();
        message.content = Some(format!("Hello from {}", name.unwrap_or_default()));
        Reply::Response(InteractionResponse::message(message))
    }

    async fn webhook_event(&self, _event: WebhookEventBody) {
        self.events.fetch_add(1, Ordering::SeqCst);
    }
}

fn signing_key() -> SigningKey {
    SigningKey::from_bytes(&[7; 32])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn endpoint(events: Arc<AtomicUsize>) -> Endpoint<Bot> {
    let public = hex(signing_key().verifying_key().as_bytes());
    Endpoint::new(PublicKey::from_hex(&public).unwrap(), Bot { events })
}

fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string()
}

fn sign(timestamp: &str, body: &str) -> String {
    let message = format!("{timestamp}{body}");
    hex(&signing_key().sign(message.as_bytes()).to_bytes())
}

fn command(name: &str) -> String {
    json!({
        "id": "1", "application_id": "2", "type": 2, "token": "abc", "version": 1,
        "data": {"id": "3", "name": name, "type": 1},
        "channel_id": "4", "user": {"id": "5", "username": "u", "discriminator": "0", "avatar": null},
        "app_permissions": "0", "locale": "en-US", "entitlements": [], "authorizing_integration_owners": {}, "attachment_size_limit": 0
    })
    .to_string()
}

async fn call(endpoint: &Endpoint<Bot>, body: &str) -> (u16, Value) {
    let timestamp = now();
    let signature = sign(&timestamp, body);
    let response = endpoint
        .handle(Some(&signature), Some(&timestamp), body.as_bytes())
        .await;
    let value = serde_json::from_slice(&response.body).unwrap_or(Value::Null);
    (response.status, value)
}

#[tokio::test]
async fn answers_ping_with_pong() {
    let endpoint = endpoint(Arc::default());
    let ping = json!({"id": "1", "application_id": "2", "type": 1, "token": "abc", "version": 1})
        .to_string();

    let (status, body) = call(&endpoint, &ping).await;

    assert_eq!(status, 200);
    assert_eq!(body, json!({"type": 1}));
}

#[tokio::test]
async fn answers_commands_inline() {
    let endpoint = endpoint(Arc::default());

    let (status, body) = call(&endpoint, &command("hello")).await;

    assert_eq!(status, 200);
    assert_eq!(
        body,
        json!({"type": 4, "data": {"content": "Hello from hello"}})
    );
}

#[tokio::test]
async fn answers_202_after_acknowledgement() {
    let endpoint = endpoint(Arc::default());

    let (status, _) = call(&endpoint, &command("defer")).await;

    assert_eq!(status, 202);
}

#[tokio::test]
async fn refuses_bad_signatures() {
    let endpoint = endpoint(Arc::default());
    let body = command("hello");
    let timestamp = now();
    let signature = sign(&timestamp, &body);

    let tampered = endpoint
        .handle(
            Some(&signature),
            Some(&timestamp),
            command("other").as_bytes(),
        )
        .await;
    let missing = endpoint
        .handle(None, Some(&timestamp), body.as_bytes())
        .await;
    let garbage = endpoint
        .handle(Some("zz"), Some(&timestamp), body.as_bytes())
        .await;

    assert_eq!(tampered.status, 401);
    assert_eq!(missing.status, 401);
    assert_eq!(garbage.status, 401);
}

#[tokio::test]
async fn refuses_stale_timestamps() {
    let endpoint = endpoint(Arc::default());
    let body = command("hello");
    let old = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        - 3600)
        .to_string();

    let stale = endpoint
        .handle(Some(&sign(&old, &body)), Some(&old), body.as_bytes())
        .await;
    let lenient = endpoint
        .clone()
        .max_age(None)
        .handle(Some(&sign(&old, &body)), Some(&old), body.as_bytes())
        .await;

    assert_eq!(stale.status, 401);
    assert_eq!(lenient.status, 200);
}

#[tokio::test]
async fn acknowledges_webhook_events_and_runs_the_handler() {
    let events = Arc::new(AtomicUsize::new(0));
    let endpoint = endpoint(Arc::clone(&events));
    let ping = json!({"version": 1, "application_id": "2", "type": 0}).to_string();
    let event = json!({
        "version": 1, "application_id": "2", "type": 1,
        "event": {"type": "APPLICATION_DEAUTHORIZED", "timestamp": "2026-01-01T00:00:00.000000+00:00",
                  "data": {"user": {"id": "5", "username": "u", "discriminator": "0", "avatar": null}}}
    })
    .to_string();

    let (ping_status, _) = call(&endpoint, &ping).await;
    let (event_status, _) = call(&endpoint, &event).await;

    assert_eq!(ping_status, 204);
    assert_eq!(event_status, 204);
    for _ in 0..50 {
        if events.load(Ordering::SeqCst) == 1 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the webhook event handler did not run");
}
