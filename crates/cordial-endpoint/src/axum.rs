//! An `axum` router for the endpoint.
//!
//! ```no_run
//! # use cordial_endpoint::{Endpoint, Handler, PublicKey, Reply};
//! # use cordial_model::interaction::Interaction;
//! # struct Bot;
//! # impl Handler for Bot {
//! #     async fn interaction(&self, _interaction: Interaction) -> Reply { Reply::Acknowledged }
//! # }
//! # async fn run(key: PublicKey) {
//! let app = axum::Router::new().nest("/discord", cordial_endpoint::axum::router(Endpoint::new(key, Bot)));
//! let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
//! axum::serve(listener, app).await.unwrap();
//! # }
//! ```

use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Response, StatusCode, header};
use axum::routing::post;

use crate::{Endpoint, Handler, SIGNATURE_HEADER, TIMESTAMP_HEADER};

/// Makes a router that handles `POST /` with the endpoint.
pub fn router<H: Handler>(endpoint: Endpoint<H>) -> axum::Router {
    axum::Router::new()
        .route("/", post(handle::<H>))
        .with_state(endpoint)
}

async fn handle<H: Handler>(
    State(endpoint): State<Endpoint<H>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response<Body> {
    let signature = headers
        .get(SIGNATURE_HEADER)
        .and_then(|value| value.to_str().ok());
    let timestamp = headers
        .get(TIMESTAMP_HEADER)
        .and_then(|value| value.to_str().ok());

    let answer = endpoint.handle(signature, timestamp, &body).await;

    let mut response = Response::new(Body::from(answer.body));
    *response.status_mut() =
        StatusCode::from_u16(answer.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    if let Some(content_type) = answer.content_type {
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static(content_type),
        );
    }
    response
}
