//! Receives Discord interactions and webhook events over HTTP.
//!
//! Set the "Interactions Endpoint URL" or the "Event Webhooks URL" of the application to a
//! URL that this crate serves. Each request is checked with the public key of the application.
//! See `docs/ARCHITECTURE.md`, section 10.
//!
//! The core is independent of the HTTP server: give the two signature headers and the raw
//! body to [`Endpoint::handle`], and send back the [`HttpResponse`]. The `axum` feature gives
//! a ready router.
//!
//! ```
//! use cordial_endpoint::{Endpoint, Handler, PublicKey, Reply};
//! use cordial_model::interaction::{Interaction, InteractionMessage, InteractionResponse};
//!
//! struct Bot;
//!
//! impl Handler for Bot {
//!     async fn interaction(&self, _interaction: Interaction) -> Reply {
//!         let mut message = InteractionMessage::default();
//!         message.content = Some("Hello".to_owned());
//!         Reply::Response(InteractionResponse::message(message))
//!     }
//! }
//!
//! let key = PublicKey::from_hex("3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c").unwrap();
//! let endpoint = Endpoint::new(key, Bot);
//! ```

#[cfg(feature = "axum")]
pub mod axum;
mod verify;

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use cordial_model::interaction::{Interaction, InteractionResponse, InteractionType};
use cordial_model::webhook_event::{WebhookEventBody, WebhookEventPayload, WebhookPayloadType};
use serde::Deserialize;

pub use verify::{PublicKey, SIGNATURE_HEADER, TIMESTAMP_HEADER, VerifyError, check_timestamp};

/// The default maximum age of a request timestamp.
pub const DEFAULT_MAX_AGE: Duration = Duration::from_secs(300);

/// What a handler answers to an interaction.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Reply {
    /// Answer the HTTP request with this response. Discord needs it within 3 seconds.
    ///
    /// The inline response cannot carry files. Use the callback endpoint for files.
    Response(InteractionResponse),
    /// The handler already answered with the callback endpoint. The HTTP answer is 202.
    Acknowledged,
}

/// Handles the requests that Discord sends.
pub trait Handler: Send + Sync + 'static {
    /// Handles an interaction. PING never arrives here: the endpoint answers it.
    fn interaction(&self, interaction: Interaction) -> impl Future<Output = Reply> + Send;

    /// Handles a webhook event. The endpoint already answered with 204, as Discord needs.
    fn webhook_event(&self, event: WebhookEventBody) -> impl Future<Output = ()> + Send {
        let _ = event;
        async {}
    }
}

/// The answer to an HTTP request.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct HttpResponse {
    /// The status.
    pub status: u16,
    /// The `Content-Type`, when there is a body.
    pub content_type: Option<&'static str>,
    /// The body.
    pub body: Bytes,
}

impl HttpResponse {
    fn empty(status: u16) -> Self {
        Self {
            status,
            content_type: None,
            body: Bytes::new(),
        }
    }

    fn text(status: u16, text: &'static str) -> Self {
        Self {
            status,
            content_type: Some("text/plain"),
            body: Bytes::from_static(text.as_bytes()),
        }
    }

    fn json(value: &impl serde::Serialize) -> Self {
        match serde_json::to_vec(value) {
            Ok(body) => Self {
                status: 200,
                content_type: Some("application/json"),
                body: body.into(),
            },
            Err(error) => {
                tracing::error!(%error, "cannot encode the interaction response");
                Self::text(500, "cannot encode the response")
            }
        }
    }
}

/// An HTTP endpoint for interactions and webhook events.
///
/// The endpoint checks the signature and the age of each request, answers PING, and gives
/// the rest to the [`Handler`]. Give the headers and the raw body of a request to
/// [`Endpoint::handle`], or serve the endpoint with the `axum` feature.
///
/// ```
/// use cordial_endpoint::{Endpoint, Handler, PublicKey, Reply};
/// use cordial_model::interaction::{Interaction, InteractionMessage, InteractionResponse};
///
/// struct Greeter;
///
/// impl Handler for Greeter {
///     async fn interaction(&self, _interaction: Interaction) -> Reply {
///         let message = InteractionMessage::text("Hello");
///         Reply::Response(InteractionResponse::message(message))
///     }
/// }
///
/// let key = PublicKey::from_hex("3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c")
///     .unwrap();
/// let endpoint = Endpoint::new(key, Greeter);
///
/// // A request without a valid signature gets status 401.
/// let runtime = tokio::runtime::Runtime::new().unwrap();
/// let response = runtime.block_on(endpoint.handle(None, None, b"{}"));
/// assert_eq!(response.status, 401);
/// ```
#[derive(Debug)]
pub struct Endpoint<H> {
    key: PublicKey,
    handler: Arc<H>,
    max_age: Option<Duration>,
}

impl<H> Clone for Endpoint<H> {
    fn clone(&self) -> Self {
        Self {
            key: self.key,
            handler: Arc::clone(&self.handler),
            max_age: self.max_age,
        }
    }
}

impl<H: Handler> Endpoint<H> {
    /// Makes an endpoint with the public key of the application and a handler.
    pub fn new(key: PublicKey, handler: H) -> Self {
        Self {
            key,
            handler: Arc::new(handler),
            max_age: Some(DEFAULT_MAX_AGE),
        }
    }

    /// Sets the maximum age of the request timestamp. `None` turns the check off.
    ///
    /// The check limits replays of captured requests. The default is 5 minutes.
    #[must_use]
    pub fn max_age(mut self, max_age: Option<Duration>) -> Self {
        self.max_age = max_age;
        self
    }

    /// Gives the handler.
    pub fn handler(&self) -> &Arc<H> {
        &self.handler
    }

    /// Handles one HTTP request: checks the signature, answers PING, and calls the handler.
    ///
    /// Give the values of the `X-Signature-Ed25519` and `X-Signature-Timestamp` headers and
    /// the raw body.
    pub async fn handle(
        &self,
        signature: Option<&str>,
        timestamp: Option<&str>,
        body: &[u8],
    ) -> HttpResponse {
        if let Err(error) = self.key.verify(signature, timestamp, body) {
            // Discord sends requests with bad signatures on purpose. They must get 401.
            tracing::debug!(%error, "request with a bad signature");
            return HttpResponse::text(401, "invalid request signature");
        }
        if let (Some(max_age), Some(timestamp)) = (self.max_age, timestamp)
            && check_timestamp(timestamp, max_age).is_err()
        {
            return HttpResponse::text(401, "stale request timestamp");
        }

        #[derive(Deserialize)]
        struct Probe {
            #[serde(default)]
            token: Option<serde::de::IgnoredAny>,
        }
        let Ok(probe) = serde_json::from_slice::<Probe>(body) else {
            return HttpResponse::text(400, "the body is not JSON");
        };

        if probe.token.is_some() {
            self.handle_interaction(body).await
        } else {
            self.handle_webhook_event(body)
        }
    }

    async fn handle_interaction(&self, body: &[u8]) -> HttpResponse {
        let interaction: Interaction = match serde_json::from_slice(body) {
            Ok(interaction) => interaction,
            Err(error) => {
                tracing::warn!(%error, "cannot decode an interaction");
                return HttpResponse::text(400, "cannot decode the interaction");
            }
        };

        if interaction.kind == InteractionType::Ping {
            return HttpResponse::json(&InteractionResponse::pong());
        }

        match self.handler.interaction(interaction).await {
            Reply::Response(response) => HttpResponse::json(&response),
            Reply::Acknowledged => HttpResponse::empty(202),
        }
    }

    fn handle_webhook_event(&self, body: &[u8]) -> HttpResponse {
        let payload: WebhookEventPayload = match serde_json::from_slice(body) {
            Ok(payload) => payload,
            Err(error) => {
                tracing::warn!(%error, "cannot decode a webhook event");
                return HttpResponse::text(400, "cannot decode the webhook event");
            }
        };

        // Discord needs 204 within 3 seconds. The handler runs after the answer.
        if payload.kind == WebhookPayloadType::Event
            && let Some(event) = payload.event
        {
            let handler = Arc::clone(&self.handler);
            tokio::spawn(async move { handler.webhook_event(event).await });
        }
        HttpResponse::empty(204)
    }
}
