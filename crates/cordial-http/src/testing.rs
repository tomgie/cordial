//! A fake transport for tests. Turn on the `testing` feature to use it in other crates.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};

use bytes::Bytes;

use crate::error::TransportError;
use crate::transport::{BoxFuture, Transport};

/// A response that the fake transport gives.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct FakeResponse {
    /// The status.
    pub status: u16,
    /// The headers.
    pub headers: Vec<(String, String)>,
    /// The body.
    pub body: Bytes,
}

impl FakeResponse {
    /// Makes a response with a JSON body.
    #[must_use]
    pub fn json(status: u16, body: &serde_json::Value) -> Self {
        Self {
            status,
            headers: vec![("content-type".to_owned(), "application/json".to_owned())],
            body: Bytes::from(body.to_string()),
        }
    }

    /// Makes a response with no body.
    #[must_use]
    pub fn empty(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: Bytes::new(),
        }
    }

    /// Adds a header.
    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    /// Adds rate limit headers.
    #[must_use]
    pub fn rate_limit(self, limit: u32, remaining: u32, reset_after: f64, bucket: &str) -> Self {
        self.header("x-ratelimit-limit", &limit.to_string())
            .header("x-ratelimit-remaining", &remaining.to_string())
            .header("x-ratelimit-reset-after", &reset_after.to_string())
            .header("x-ratelimit-bucket", bucket)
    }
}

/// A request that the fake transport received.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct RecordedRequest {
    /// The method.
    pub method: String,
    /// The full URL.
    pub url: String,
    /// The headers.
    pub headers: http::HeaderMap,
    /// The body.
    pub body: Bytes,
}

impl RecordedRequest {
    /// Gives a header value as text.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    /// Decodes the body as JSON.
    ///
    /// # Panics
    ///
    /// Panics when the body is not JSON.
    #[must_use]
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("the body is not JSON")
    }
}

/// A transport that gives scripted responses and records requests.
#[derive(Clone, Default)]
pub struct FakeTransport {
    responses: Arc<Mutex<VecDeque<Result<FakeResponse, String>>>>,
    requests: Arc<Mutex<Vec<RecordedRequest>>>,
}

impl fmt::Debug for FakeTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FakeTransport")
            .finish_non_exhaustive()
    }
}

impl FakeTransport {
    /// Makes a fake transport with no responses.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a response to the end of the script.
    pub fn push(&self, response: FakeResponse) {
        lock(&self.responses).push_back(Ok(response));
    }

    /// Adds a connection error to the end of the script.
    pub fn push_error(&self, message: &str) {
        lock(&self.responses).push_back(Err(message.to_owned()));
    }

    /// Gives the requests that arrived.
    #[must_use]
    pub fn requests(&self) -> Vec<RecordedRequest> {
        lock(&self.requests).clone()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Transport for FakeTransport {
    fn send(
        &self,
        request: http::Request<Bytes>,
    ) -> BoxFuture<'static, Result<http::Response<Bytes>, TransportError>> {
        let (parts, body) = request.into_parts();
        lock(&self.requests).push(RecordedRequest {
            method: parts.method.to_string(),
            url: parts.uri.to_string(),
            headers: parts.headers,
            body,
        });

        let next = lock(&self.responses).pop_front();
        Box::pin(async move {
            let response = next
                .unwrap_or_else(|| Err("the fake transport has no more responses".to_owned()))
                .map_err(|message| TransportError::Connection(message.into()))?;

            let mut builder = http::Response::builder().status(response.status);
            for (name, value) in &response.headers {
                builder = builder.header(name, value);
            }
            builder
                .body(response.body)
                .map_err(|error| TransportError::InvalidRequest(error.to_string()))
        })
    }
}
