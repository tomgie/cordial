//! The HTTP client.

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use serde::de::DeserializeOwned;
use tokio::time::Instant;
use tracing::Instrument;

use crate::error::{ApiError, Error, TransportError};
use crate::ratelimit::{
    InMemoryRateLimiter, Outcome, RateLimitHeaders, RateLimitScope, RateLimiter,
};
use crate::request::{Endpoint, RequestBody};
use crate::route::{Authentication, Route};
use crate::token::Token;
use crate::transport::Transport;

/// The base URL of the API, with the version.
pub const API_BASE: &str = "https://discord.com/api/v10";

/// The window of the invalid request limit.
const INVALID_WINDOW: Duration = Duration::from_secs(600);
/// The number of invalid requests in the window at which the client warns. Discord bans the
/// IP address at 10,000.
const INVALID_WARN_THRESHOLD: usize = 5000;
/// The number of invalid requests in the window at which the client slows down.
const INVALID_SLOW_THRESHOLD: usize = 9000;

/// A client for the HTTP API of Discord.
///
/// The client is cheap to clone. All clones share the rate limiter and the connection pool.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    token: Option<Token>,
    base_url: String,
    user_agent: String,
    transport: Arc<dyn Transport>,
    rate_limiter: Arc<dyn RateLimiter>,
    timeout: Duration,
    max_retries: u32,
    max_rate_limit_retries: u32,
    validate: bool,
    token_invalid: AtomicBool,
    invalid_requests: Mutex<VecDeque<Instant>>,
}

impl fmt::Debug for Client {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Client")
            .field("token", &self.inner.token)
            .field("base_url", &self.inner.base_url)
            .field("rate_limiter", &self.inner.rate_limiter)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Makes a client with a bot token and the default settings.
    ///
    /// # Panics
    ///
    /// Panics without a default transport (the features `rustls` and `native-tls` are off).
    #[must_use]
    pub fn new(token: Token) -> Self {
        Self::builder().token(token).build()
    }

    /// Makes a builder for a client with other settings.
    #[must_use]
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Gives the token.
    #[must_use]
    pub fn token(&self) -> Option<&Token> {
        self.inner.token.as_ref()
    }

    /// Tells if the client runs the client-side checks of documented limits.
    #[must_use]
    pub fn validates(&self) -> bool {
        self.inner.validate
    }

    /// Gives the number of invalid requests (401, 403, 429) in the last 10 minutes.
    #[must_use]
    pub fn invalid_request_count(&self) -> usize {
        let mut times = lock(&self.inner.invalid_requests);
        prune_window(&mut times, Instant::now());
        times.len()
    }

    /// Sends a request and decodes the response.
    ///
    /// # Errors
    ///
    /// Gives an error when the request is not valid, the transport fails, or Discord answers
    /// with an error status.
    pub async fn request<E: Endpoint>(&self, endpoint: &E) -> Result<E::Response, Error> {
        let route = endpoint.route();
        let (status, body) = self.send(endpoint, &route).await?;
        decode_body(&route, status, &body)
    }

    /// Sends a request and gives the raw response body.
    ///
    /// # Errors
    ///
    /// Gives an error when the request is not valid, the transport fails, or Discord answers
    /// with an error status.
    pub async fn request_raw<E: Endpoint>(&self, endpoint: &E) -> Result<Bytes, Error> {
        let route = endpoint.route();
        self.send(endpoint, &route).await.map(|(_, body)| body)
    }

    async fn send<E: Endpoint>(&self, endpoint: &E, route: &Route) -> Result<(u16, Bytes), Error> {
        if self.inner.token_invalid.load(Ordering::Relaxed) {
            return Err(Error::InvalidToken);
        }

        let body = endpoint.body(self.inner.validate)?;
        let retry_safe = endpoint.retry_safe();
        let reason = endpoint.audit_reason();
        let extra_headers = endpoint.headers();
        let mut attempt = 0;
        let mut rate_limit_attempt = 0;

        loop {
            self.slow_down_if_near_ban().await;

            let ticket = self.inner.rate_limiter.acquire(route).await;
            let request = self.build_request(route, &body, reason, &extra_headers)?;

            let span = tracing::debug_span!("request", method = %route.method, route = route.template, attempt);
            let sending =
                tokio::time::timeout(self.inner.timeout, self.inner.transport.send(request));

            let response = match sending.instrument(span).await {
                Ok(Ok(response)) => response,
                Ok(Err(error)) => {
                    ticket.complete(Outcome::Failed);
                    if retry_safe && attempt < self.inner.max_retries {
                        attempt += 1;
                        tracing::warn!(%error, attempt, "transport error; retrying");
                        tokio::time::sleep(backoff(attempt)).await;
                        continue;
                    }
                    return Err(Error::Transport(error));
                }
                Err(_) => {
                    ticket.complete(Outcome::Failed);
                    let error = TransportError::Timeout(self.inner.timeout);
                    if retry_safe && attempt < self.inner.max_retries {
                        attempt += 1;
                        tracing::warn!(%error, attempt, "timeout; retrying");
                        tokio::time::sleep(backoff(attempt)).await;
                        continue;
                    }
                    return Err(Error::Transport(error));
                }
            };

            let status = response.status().as_u16();
            let mut headers = RateLimitHeaders::parse(response.headers());
            let body_bytes = response.into_body();

            match status {
                200..=299 => {
                    ticket.complete(Outcome::Response {
                        status,
                        headers: &headers,
                    });
                    return Ok((status, body_bytes));
                }
                429 => {
                    add_rate_limit_body(&mut headers, &body_bytes);
                    ticket.complete(Outcome::Response {
                        status,
                        headers: &headers,
                    });

                    let shared = headers.scope == Some(RateLimitScope::Shared);
                    if !shared {
                        self.record_invalid_request();
                    }

                    let retry_after = headers.retry_after.unwrap_or(Duration::from_secs(1));
                    let global = headers.global || headers.scope == Some(RateLimitScope::Global);
                    tracing::warn!(retry_after_ms = retry_after.as_millis(), global, ?headers.scope, "rate limited");

                    if rate_limit_attempt < self.inner.max_rate_limit_retries {
                        rate_limit_attempt += 1;
                        // The rate limiter already holds the next request back for the bucket or
                        // the global pause.
                        continue;
                    }
                    return Err(Error::RateLimited {
                        route: route.template,
                        retry_after,
                        global,
                    });
                }
                _ => {
                    ticket.complete(Outcome::Response {
                        status,
                        headers: &headers,
                    });
                    if status == 401 && route.authentication == Authentication::Token {
                        tracing::error!(
                            "status 401: the token is not valid; stopping all requests with it"
                        );
                        self.inner.token_invalid.store(true, Ordering::Relaxed);
                    }
                    if status == 401 || status == 403 {
                        self.record_invalid_request();
                    }

                    let server_error = matches!(status, 500 | 502 | 503 | 504);
                    if server_error && retry_safe && attempt < self.inner.max_retries {
                        attempt += 1;
                        tracing::warn!(status, attempt, "server error; retrying");
                        tokio::time::sleep(backoff(attempt)).await;
                        continue;
                    }

                    let error =
                        ApiError::from_response(status, route.method, route.template, &body_bytes);
                    return Err(Error::Api(Box::new(error)));
                }
            }
        }
    }

    fn build_request(
        &self,
        route: &Route,
        body: &RequestBody,
        reason: Option<&str>,
        extra_headers: &[(&'static str, String)],
    ) -> Result<http::Request<Bytes>, Error> {
        let url = format!("{}{}", self.inner.base_url, route.path);
        let mut builder = http::Request::builder()
            .method(http::Method::from(route.method))
            .uri(url)
            .header(http::header::USER_AGENT, &self.inner.user_agent);

        if route.authentication == Authentication::Token {
            let token = self.inner.token.as_ref().ok_or(Error::MissingToken)?;
            builder = builder.header(http::header::AUTHORIZATION, token.authorization());
        }

        if let Some(reason) = reason {
            let encoded =
                percent_encoding::utf8_percent_encode(reason, crate::QUERY_ENCODE_SET).to_string();
            builder = builder.header("X-Audit-Log-Reason", encoded);
        }

        for (name, value) in extra_headers {
            builder = builder.header(*name, value);
        }

        let body_bytes = match body.encode() {
            Some((content_type, bytes)) => {
                builder = builder.header(http::header::CONTENT_TYPE, content_type);
                bytes
            }
            None => Bytes::new(),
        };

        builder
            .body(body_bytes)
            .map_err(|error| Error::Transport(TransportError::InvalidRequest(error.to_string())))
    }

    fn record_invalid_request(&self) {
        let now = Instant::now();
        let mut times = lock(&self.inner.invalid_requests);
        prune_window(&mut times, now);
        times.push_back(now);

        if times.len() == INVALID_WARN_THRESHOLD {
            tracing::warn!(
                count = times.len(),
                "many invalid requests (401, 403, 429) in 10 minutes; Discord bans the IP at 10,000"
            );
        }
    }

    async fn slow_down_if_near_ban(&self) {
        let count = self.invalid_request_count();
        if count >= INVALID_SLOW_THRESHOLD {
            tracing::error!(count, "near the invalid request limit; slowing down");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn prune_window(times: &mut VecDeque<Instant>, now: Instant) {
    while times
        .front()
        .is_some_and(|time| now.duration_since(*time) > INVALID_WINDOW)
    {
        times.pop_front();
    }
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(500 * 2_u64.pow(attempt.min(6)))
}

/// Adds the data of a 429 body to the headers, when the headers do not have it.
fn add_rate_limit_body(headers: &mut RateLimitHeaders, body: &[u8]) {
    #[derive(serde::Deserialize)]
    struct Body {
        #[serde(default)]
        retry_after: Option<f64>,
        #[serde(default)]
        global: bool,
    }

    if let Ok(parsed) = serde_json::from_slice::<Body>(body) {
        if headers.retry_after.is_none() {
            headers.retry_after = parsed
                .retry_after
                .and_then(|seconds| Duration::try_from_secs_f64(seconds.max(0.0)).ok());
        }
        headers.global |= parsed.global;
    }
}

fn decode_body<T: DeserializeOwned>(route: &Route, status: u16, body: &[u8]) -> Result<T, Error> {
    let text: &[u8] = if status == 204 || body.is_empty() {
        b"null"
    } else {
        body
    };
    serde_json::from_slice(text).map_err(|source| Error::Decode {
        method: route.method,
        route: route.template,
        source,
        body_start: String::from_utf8_lossy(&body[..body.len().min(300)]).into_owned(),
    })
}

/// A builder for a [`Client`].
#[derive(Default)]
pub struct ClientBuilder {
    token: Option<Token>,
    base_url: Option<String>,
    user_agent_suffix: Option<String>,
    transport: Option<Arc<dyn Transport>>,
    rate_limiter: Option<Arc<dyn RateLimiter>>,
    timeout: Option<Duration>,
    max_retries: Option<u32>,
    max_rate_limit_retries: Option<u32>,
    no_validation: bool,
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClientBuilder")
            .field("token", &self.token)
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl ClientBuilder {
    /// Sets the token.
    #[must_use]
    pub fn token(mut self, token: Token) -> Self {
        self.token = Some(token);
        self
    }

    /// Sets the base URL, for example the URL of a rate limit proxy. The default is
    /// `https://discord.com/api/v10`.
    #[must_use]
    pub fn base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into().trim_end_matches('/').to_owned());
        self
    }

    /// Adds text to the `User-Agent` header, for example the name of the bot.
    #[must_use]
    pub fn user_agent_suffix(mut self, suffix: impl Into<String>) -> Self {
        self.user_agent_suffix = Some(suffix.into());
        self
    }

    /// Sets the transport.
    #[must_use]
    pub fn transport(mut self, transport: impl Transport) -> Self {
        self.transport = Some(Arc::new(transport));
        self
    }

    /// Sets the rate limiter. The default is [`InMemoryRateLimiter`].
    #[must_use]
    pub fn rate_limiter(mut self, rate_limiter: impl RateLimiter) -> Self {
        self.rate_limiter = Some(Arc::new(rate_limiter));
        self
    }

    /// Sets the time limit for one request. The default is 30 seconds.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the number of retries after a server error or a connection error. The default is 3.
    #[must_use]
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = Some(retries);
        self
    }

    /// Sets the number of retries after status 429. The default is 5. With 0, the client
    /// gives [`Error::RateLimited`] at once.
    #[must_use]
    pub fn max_rate_limit_retries(mut self, retries: u32) -> Self {
        self.max_rate_limit_retries = Some(retries);
        self
    }

    /// Turns off the client-side checks of documented limits.
    ///
    /// Use this only when Discord changes a limit before this library does.
    #[must_use]
    pub fn without_validation(mut self) -> Self {
        self.no_validation = true;
        self
    }

    /// Makes the client.
    ///
    /// # Panics
    ///
    /// Panics when no transport is set and no default transport is compiled in.
    #[must_use]
    pub fn build(self) -> Client {
        let transport = self.transport.unwrap_or_else(default_transport);
        let mut user_agent = format!(
            "DiscordBot (https://crates.io/crates/cordial, {})",
            env!("CARGO_PKG_VERSION")
        );
        if let Some(suffix) = self.user_agent_suffix {
            user_agent.push(' ');
            user_agent.push_str(&suffix);
        }

        Client {
            inner: Arc::new(Inner {
                token: self.token,
                base_url: self.base_url.unwrap_or_else(|| API_BASE.to_owned()),
                user_agent,
                transport,
                rate_limiter: self
                    .rate_limiter
                    .unwrap_or_else(|| Arc::new(InMemoryRateLimiter::new())),
                timeout: self.timeout.unwrap_or(Duration::from_secs(30)),
                max_retries: self.max_retries.unwrap_or(3),
                max_rate_limit_retries: self.max_rate_limit_retries.unwrap_or(5),
                validate: !self.no_validation,
                token_invalid: AtomicBool::new(false),
                invalid_requests: Mutex::new(VecDeque::new()),
            }),
        }
    }
}

#[cfg(feature = "reqwest-transport")]
fn default_transport() -> Arc<dyn Transport> {
    Arc::new(crate::transport::ReqwestTransport::new())
}

#[cfg(not(feature = "reqwest-transport"))]
fn default_transport() -> Arc<dyn Transport> {
    panic!(
        "no transport: turn on the `rustls` or `native-tls` feature, or call `ClientBuilder::transport`"
    )
}
