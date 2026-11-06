//! The interaction endpoint: signature checks and the decode of request bodies.

use std::fmt::Write as _;
use std::future::Future;

use cordial_endpoint::{DEFAULT_MAX_AGE, Endpoint, Handler, PublicKey, Reply, check_timestamp};
use cordial_model::interaction::Interaction;
use cordial_model::webhook_event::{WebhookEventBody, WebhookEventPayload};
use ed25519_dalek::{Signer, SigningKey};

use crate::block_on;
use crate::reader::Reader;

/// The fixed signing key of the test application.
const SECRET_KEY: [u8; 32] = [7; 32];

/// The timestamp of each signed request. The body turns the age check off.
const TIMESTAMP: &str = "1700000000";

struct FuzzHandler;

impl Handler for FuzzHandler {
    fn interaction(&self, interaction: Interaction) -> impl Future<Output = Reply> + Send {
        drop(interaction);
        async { Reply::Acknowledged }
    }
}

/// An HTTP request to the interaction endpoint.
///
/// The rest after the selector byte is the body. With selector bit 0 clear, the body signs
/// the request with the fixed key, so the request passes the signature check. With the bit
/// set, the start of the body is the signature, so the check fails.
pub fn endpoint(data: &[u8]) {
    let mut reader = Reader::new(data);
    let selector = reader.byte();
    let body = reader.rest();

    decode_bodies(body);
    let text = String::from_utf8_lossy(body);
    let _ = check_timestamp(&text, DEFAULT_MAX_AGE);
    let _ = PublicKey::from_hex(&text);

    let signing_key = SigningKey::from_bytes(&SECRET_KEY);
    let key = PublicKey::from_bytes(&signing_key.verifying_key().to_bytes())
        .expect("the fixed key is valid");
    let endpoint = Endpoint::new(key, FuzzHandler).max_age(None);

    let signed = selector & 1 == 0;
    let signature = if signed {
        let message = [TIMESTAMP.as_bytes(), body].concat();
        hex(&signing_key.sign(&message).to_bytes())
    } else {
        text.chars().take(128).collect()
    };

    let response = block_on(endpoint.handle(Some(&signature), Some(TIMESTAMP), body));
    if signed {
        assert_ne!(response.status, 401, "a signed request must pass the check");
    }
}

fn decode_bodies(body: &[u8]) {
    let _ = serde_json::from_slice::<Interaction>(body);
    let _ = serde_json::from_slice::<WebhookEventPayload>(body);
    if let Ok(event) = serde_json::from_slice::<WebhookEventBody>(body) {
        let _ = event.decode();
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}
