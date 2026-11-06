//! Rate limits of the HTTP API.
//!
//! See `docs/research/02-http.md`, section 2, and `docs/ARCHITECTURE.md`, section 6.3.
//!
//! The [`InMemoryRateLimiter`] keeps one bucket for each bucket hash and major parameter.
//! Before Discord tells the hash of a route, the route has a temporary bucket that lets only
//! one request through at a time. A separate limit applies to all requests (50 per second).

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::sync::OwnedMutexGuard;
use tokio::time::Instant;

use crate::route::{MajorParameter, Method, Route, RouteKey};
use crate::transport::BoxFuture;

/// The default number of requests for each second for the global limit.
pub const DEFAULT_GLOBAL_LIMIT: u32 = 50;

/// The rate limit data of a response.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct RateLimitHeaders {
    /// `X-RateLimit-Limit`.
    pub limit: Option<u32>,
    /// `X-RateLimit-Remaining`.
    pub remaining: Option<u32>,
    /// `X-RateLimit-Reset-After`.
    pub reset_after: Option<Duration>,
    /// `X-RateLimit-Bucket`.
    pub bucket: Option<String>,
    /// `X-RateLimit-Global`, or `global` in a 429 body.
    pub global: bool,
    /// `X-RateLimit-Scope`.
    pub scope: Option<RateLimitScope>,
    /// `Retry-After`, or `retry_after` in a 429 body.
    pub retry_after: Option<Duration>,
}

impl RateLimitHeaders {
    /// Reads the rate limit headers of a response.
    #[must_use]
    pub fn parse(headers: &http::HeaderMap) -> Self {
        fn text<'a>(headers: &'a http::HeaderMap, name: &str) -> Option<&'a str> {
            headers.get(name).and_then(|value| value.to_str().ok())
        }

        fn seconds(headers: &http::HeaderMap, name: &str) -> Option<Duration> {
            text(headers, name)
                .and_then(|value| value.trim().parse::<f64>().ok())
                .and_then(|value| Duration::try_from_secs_f64(value.max(0.0)).ok())
        }

        Self {
            limit: text(headers, "x-ratelimit-limit").and_then(|value| value.parse().ok()),
            remaining: text(headers, "x-ratelimit-remaining").and_then(|value| value.parse().ok()),
            reset_after: seconds(headers, "x-ratelimit-reset-after"),
            bucket: text(headers, "x-ratelimit-bucket").map(str::to_owned),
            global: text(headers, "x-ratelimit-global")
                .is_some_and(|value| value.eq_ignore_ascii_case("true")),
            scope: text(headers, "x-ratelimit-scope").and_then(RateLimitScope::parse),
            retry_after: seconds(headers, "retry-after"),
        }
    }

    /// Tells if the response has rate limit data for the bucket.
    #[must_use]
    pub fn has_bucket_data(&self) -> bool {
        self.limit.is_some() && self.remaining.is_some() && self.reset_after.is_some()
    }
}

/// The scope of a 429 response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RateLimitScope {
    /// The limit of this bot on this route.
    User,
    /// The global limit of this bot.
    Global,
    /// A limit that all users of the resource share. It is not a fault of the bot.
    Shared,
}

impl RateLimitScope {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "user" => Some(Self::User),
            "global" => Some(Self::Global),
            "shared" => Some(Self::Shared),
            _ => None,
        }
    }
}

/// What happened to a request after its ticket was given.
#[derive(Debug)]
#[non_exhaustive]
pub enum Outcome<'a> {
    /// A response arrived.
    Response {
        /// The HTTP status.
        status: u16,
        /// The rate limit data.
        headers: &'a RateLimitHeaders,
    },
    /// No response arrived (connection error or timeout).
    Failed,
}

/// The permission to send one request. Give the outcome back when the request is done.
pub trait Ticket: Send {
    /// Reports the outcome of the request.
    fn complete(self: Box<Self>, outcome: Outcome<'_>);
}

/// Decides when a request can go out.
pub trait RateLimiter: Send + Sync + fmt::Debug + 'static {
    /// Waits until a request on the route can go out.
    fn acquire(&self, route: &Route) -> BoxFuture<'static, Box<dyn Ticket>>;
}

/// A rate limiter that never waits. Use it behind a proxy that does the rate limiting.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct NoRateLimiter;

struct NoTicket;

impl Ticket for NoTicket {
    fn complete(self: Box<Self>, _outcome: Outcome<'_>) {}
}

impl RateLimiter for NoRateLimiter {
    fn acquire(&self, _route: &Route) -> BoxFuture<'static, Box<dyn Ticket>> {
        Box::pin(async { Box::new(NoTicket) as Box<dyn Ticket> })
    }
}

/// The default rate limiter. It keeps all bucket data in memory.
#[derive(Clone, Debug)]
pub struct InMemoryRateLimiter {
    state: Arc<State>,
}

impl InMemoryRateLimiter {
    /// Makes a rate limiter with the default global limit (50 requests for each second).
    #[must_use]
    pub fn new() -> Self {
        Self::with_global_limit(DEFAULT_GLOBAL_LIMIT)
    }

    /// Makes a rate limiter with another global limit, for bots that Discord gave more.
    #[must_use]
    pub fn with_global_limit(requests_per_second: u32) -> Self {
        Self {
            state: Arc::new(State {
                global_limit: requests_per_second.max(1),
                global_window: tokio::sync::Mutex::new(GlobalWindow {
                    start: Instant::now(),
                    count: 0,
                }),
                global_pause: Mutex::new(None),
                hashes: Mutex::new(HashMap::new()),
                buckets: Mutex::new(HashMap::new()),
                acquisitions: AtomicU64::new(0),
            }),
        }
    }

    /// Gives the number of buckets in memory. For tests and metrics.
    #[must_use]
    pub fn bucket_count(&self) -> usize {
        lock(&self.state.buckets).len()
    }

    /// Gives the time until the global pause ends, if one is active.
    #[must_use]
    pub fn global_pause_remaining(&self) -> Option<Duration> {
        let until = (*lock(&self.state.global_pause))?;
        until.checked_duration_since(Instant::now())
    }
}

impl Default for InMemoryRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter for InMemoryRateLimiter {
    fn acquire(&self, route: &Route) -> BoxFuture<'static, Box<dyn Ticket>> {
        let state = Arc::clone(&self.state);
        let key = route.key();
        let global_limit = route.global_limit;

        Box::pin(async move {
            if global_limit {
                state.wait_for_global().await;
            }

            let bucket = state.bucket_for(&key);
            let queue_guard = Arc::clone(&bucket.queue).lock_owned().await;
            let keep_queue = bucket.wait_for_capacity().await;

            state.maybe_clean_up();

            Box::new(InMemoryTicket {
                state,
                bucket,
                key,
                queue_guard: keep_queue.then_some(queue_guard),
            }) as Box<dyn Ticket>
        })
    }
}

/// The longest wait of the limiter: about 30 years.
const MAX_WAIT: Duration = Duration::from_secs(30 * 365 * 24 * 60 * 60);

/// Gives the time `wait` after `now`.
///
/// A response can give any wait time, and `Instant + Duration` panics on overflow. Thus a
/// longer wait becomes [`MAX_WAIT`].
fn deadline(now: Instant, wait: Duration) -> Instant {
    now + wait.min(MAX_WAIT)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A panic while the lock is held leaves simple counters, which stay usable.
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

struct State {
    global_limit: u32,
    global_window: tokio::sync::Mutex<GlobalWindow>,
    global_pause: Mutex<Option<Instant>>,
    /// The bucket hash of each route (method and template). It is the same for all major
    /// parameter values.
    hashes: Mutex<HashMap<(Method, &'static str), Arc<str>>>,
    buckets: Mutex<HashMap<BucketKey, Arc<Bucket>>>,
    acquisitions: AtomicU64,
}

impl fmt::Debug for State {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("State")
            .field("global_limit", &self.global_limit)
            .field("buckets", &lock(&self.buckets).len())
            .finish_non_exhaustive()
    }
}

struct GlobalWindow {
    start: Instant,
    count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum BucketId {
    Hash(Arc<str>),
    Route(Method, &'static str),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct BucketKey {
    id: BucketId,
    major: MajorParameter,
}

impl State {
    async fn wait_for_global(&self) {
        loop {
            let pause = *lock(&self.global_pause);
            if let Some(until) = pause
                && until > Instant::now()
            {
                tokio::time::sleep_until(until).await;
                continue;
            }

            let mut window = self.global_window.lock().await;
            let now = Instant::now();
            if now.duration_since(window.start) >= Duration::from_secs(1) {
                window.start = now;
                window.count = 0;
            }
            if window.count < self.global_limit {
                window.count += 1;
                return;
            }
            let window_end = window.start + Duration::from_secs(1);
            drop(window);
            tokio::time::sleep_until(window_end).await;
        }
    }

    fn pause_global(&self, duration: Duration) {
        let until = deadline(Instant::now(), duration);
        let mut pause = lock(&self.global_pause);
        if pause.is_none_or(|current| current < until) {
            *pause = Some(until);
        }
    }

    fn bucket_for(&self, key: &RouteKey) -> Arc<Bucket> {
        let id = match lock(&self.hashes).get(&(key.method, key.template)) {
            Some(hash) => BucketId::Hash(Arc::clone(hash)),
            None => BucketId::Route(key.method, key.template),
        };
        let bucket_key = BucketKey {
            id,
            major: key.major,
        };

        let mut buckets = lock(&self.buckets);
        Arc::clone(
            buckets
                .entry(bucket_key)
                .or_insert_with(|| Arc::new(Bucket::new())),
        )
    }

    /// Stores the bucket hash of a route. The bucket that served the request also becomes the
    /// bucket of the hash, so that the limits it learned are not lost.
    fn learn_hash(&self, key: &RouteKey, hash: &str, bucket: &Arc<Bucket>) {
        let hash: Arc<str> = {
            let mut hashes = lock(&self.hashes);
            let route = (key.method, key.template);
            match hashes.get(&route) {
                Some(current) if &**current == hash => Arc::clone(current),
                _ => {
                    tracing::debug!(method = %key.method, route = key.template, bucket = hash, "learned rate limit bucket");
                    let hash: Arc<str> = Arc::from(hash);
                    hashes.insert(route, Arc::clone(&hash));
                    hash
                }
            }
        };

        let bucket_key = BucketKey {
            id: BucketId::Hash(hash),
            major: key.major,
        };
        lock(&self.buckets)
            .entry(bucket_key)
            .or_insert_with(|| Arc::clone(bucket));
    }

    fn maybe_clean_up(&self) {
        const CLEAN_UP_EVERY: u64 = 4096;
        const IDLE: Duration = Duration::from_secs(300);

        let count = self.acquisitions.fetch_add(1, Ordering::Relaxed);
        if count % CLEAN_UP_EVERY != CLEAN_UP_EVERY - 1 {
            return;
        }

        let now = Instant::now();
        lock(&self.buckets).retain(|_, bucket| {
            let in_use = Arc::strong_count(bucket) > 1;
            let idle = now.duration_since(lock(&bucket.state).last_used) > IDLE;
            in_use || !idle
        });
    }
}

struct Bucket {
    /// A FIFO lock that keeps the order of requests and holds back requests while the bucket
    /// limits are not known.
    queue: Arc<tokio::sync::Mutex<()>>,
    state: Mutex<BucketState>,
}

#[derive(Debug)]
struct BucketState {
    limit: Option<u32>,
    remaining: u32,
    reset_at: Option<Instant>,
    /// The route sends no rate limit headers: no limit applies.
    unlimited: bool,
    last_used: Instant,
}

impl Bucket {
    fn new() -> Self {
        Self {
            queue: Arc::new(tokio::sync::Mutex::new(())),
            state: Mutex::new(BucketState {
                limit: None,
                remaining: 0,
                reset_at: None,
                unlimited: false,
                last_used: Instant::now(),
            }),
        }
    }

    /// Waits until the bucket has capacity, and takes one unit.
    ///
    /// Gives `true` when the limits are not known. Then the caller keeps the queue lock until
    /// the response arrives, so that only one request is in flight.
    async fn wait_for_capacity(&self) -> bool {
        loop {
            let wait_until = {
                let mut state = lock(&self.state);
                let now = Instant::now();
                state.last_used = now;

                if state.unlimited {
                    return false;
                }
                if let Some(reset_at) = state.reset_at
                    && now >= reset_at
                {
                    state.remaining = state.limit.unwrap_or(1);
                    state.reset_at = None;
                }

                let Some(_limit) = state.limit else {
                    return true;
                };
                if state.remaining > 0 {
                    state.remaining -= 1;
                    return false;
                }
                match state.reset_at {
                    Some(reset_at) => reset_at,
                    // The bucket is empty but the reset time is not known. Wait for a response.
                    None => return true,
                }
            };

            tracing::debug!(
                wait_ms = wait_until.duration_since(Instant::now()).as_millis(),
                "bucket exhausted; waiting"
            );
            tokio::time::sleep_until(wait_until).await;
        }
    }

    fn update(&self, status: u16, headers: &RateLimitHeaders) {
        let mut state = lock(&self.state);
        let now = Instant::now();

        if let (Some(limit), Some(remaining), Some(reset_after)) =
            (headers.limit, headers.remaining, headers.reset_after)
        {
            let new_reset = deadline(now, reset_after);
            // Responses can arrive out of order. Trust the headers fully only for a new window.
            let new_window = state
                .reset_at
                .is_none_or(|current| new_reset > current + Duration::from_millis(500));
            state.limit = Some(limit);
            state.unlimited = false;
            if new_window {
                state.remaining = remaining;
                state.reset_at = Some(new_reset);
            } else {
                state.remaining = state.remaining.min(remaining);
            }
        } else if (200..300).contains(&status) && state.limit.is_none() {
            state.unlimited = true;
        }

        if status == 429 && !headers.global && headers.scope != Some(RateLimitScope::Global) {
            let retry_after = headers
                .retry_after
                .or(headers.reset_after)
                .unwrap_or(Duration::from_secs(1));
            state.limit.get_or_insert(1);
            state.unlimited = false;
            state.remaining = 0;
            state.reset_at = Some(deadline(now, retry_after));
        }
    }
}

struct InMemoryTicket {
    state: Arc<State>,
    bucket: Arc<Bucket>,
    key: RouteKey,
    queue_guard: Option<OwnedMutexGuard<()>>,
}

impl Ticket for InMemoryTicket {
    fn complete(self: Box<Self>, outcome: Outcome<'_>) {
        if let Outcome::Response { status, headers } = outcome {
            if let Some(hash) = &headers.bucket {
                self.state.learn_hash(&self.key, hash, &self.bucket);
            }
            self.bucket.update(status, headers);

            let global = headers.global || headers.scope == Some(RateLimitScope::Global);
            if status == 429 && global {
                let retry_after = headers.retry_after.unwrap_or(Duration::from_secs(1));
                tracing::warn!(
                    retry_after_ms = retry_after.as_millis(),
                    "global rate limit hit; pausing all requests"
                );
                self.state.pause_global(retry_after);
            }
        }

        drop(self.queue_guard);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::route::Route;

    fn route(channel: u64) -> Route {
        Route::new(
            Method::Post,
            "/channels/{channel_id}/messages",
            format!("/channels/{channel}/messages"),
            MajorParameter::Channel(cordial_model::id::ChannelId::new_const(channel)),
        )
    }

    fn headers(limit: u32, remaining: u32, reset_after_secs: f64) -> RateLimitHeaders {
        RateLimitHeaders {
            limit: Some(limit),
            remaining: Some(remaining),
            reset_after: Some(Duration::from_secs_f64(reset_after_secs)),
            bucket: Some("abcd".to_owned()),
            ..RateLimitHeaders::default()
        }
    }

    #[test]
    fn parses_headers() {
        let mut map = http::HeaderMap::new();
        map.insert("x-ratelimit-limit", "5".parse().unwrap());
        map.insert("x-ratelimit-remaining", "4".parse().unwrap());
        map.insert("x-ratelimit-reset-after", "1.250".parse().unwrap());
        map.insert("x-ratelimit-bucket", "abcd1234".parse().unwrap());
        map.insert("x-ratelimit-scope", "shared".parse().unwrap());

        let headers = RateLimitHeaders::parse(&map);

        assert_eq!(headers.limit, Some(5));
        assert_eq!(headers.remaining, Some(4));
        assert_eq!(headers.reset_after, Some(Duration::from_millis(1250)));
        assert_eq!(headers.bucket.as_deref(), Some("abcd1234"));
        assert_eq!(headers.scope, Some(RateLimitScope::Shared));
        assert!(headers.has_bucket_data());
    }

    #[tokio::test(start_paused = true)]
    async fn unknown_bucket_lets_one_request_through() {
        let limiter = InMemoryRateLimiter::new();
        let first = limiter.acquire(&route(1)).await;

        // A second request waits until the first one completes.
        let second = limiter.acquire(&route(1));
        tokio::pin!(second);
        assert!(
            tokio::time::timeout(Duration::from_millis(10), &mut second)
                .await
                .is_err()
        );

        first.complete(Outcome::Response {
            status: 200,
            headers: &headers(5, 4, 5.0),
        });
        let second = tokio::time::timeout(Duration::from_millis(10), second).await;
        assert!(second.is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn waits_for_reset_when_bucket_is_empty() {
        let limiter = InMemoryRateLimiter::new();
        limiter
            .acquire(&route(1))
            .await
            .complete(Outcome::Response {
                status: 200,
                headers: &headers(1, 0, 2.0),
            });

        let start = Instant::now();
        let ticket = limiter.acquire(&route(1)).await;

        assert!(start.elapsed() >= Duration::from_secs(2));
        ticket.complete(Outcome::Response {
            status: 200,
            headers: &headers(1, 0, 2.0),
        });
    }

    #[tokio::test(start_paused = true)]
    async fn major_parameters_have_separate_buckets() {
        let limiter = InMemoryRateLimiter::new();
        limiter
            .acquire(&route(1))
            .await
            .complete(Outcome::Response {
                status: 200,
                headers: &headers(1, 0, 60.0),
            });

        // Channel 2 has its own bucket, so it does not wait for channel 1.
        let other =
            tokio::time::timeout(Duration::from_millis(10), limiter.acquire(&route(2))).await;

        assert!(other.is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn global_429_pauses_all_routes() {
        let limiter = InMemoryRateLimiter::new();
        let global = RateLimitHeaders {
            global: true,
            retry_after: Some(Duration::from_secs(3)),
            ..RateLimitHeaders::default()
        };
        limiter
            .acquire(&route(1))
            .await
            .complete(Outcome::Response {
                status: 429,
                headers: &global,
            });

        let start = Instant::now();
        let _ticket = limiter.acquire(&route(9)).await;

        assert!(start.elapsed() >= Duration::from_secs(3));
    }

    #[tokio::test(start_paused = true)]
    async fn route_without_headers_becomes_unlimited() {
        let limiter = InMemoryRateLimiter::new();
        limiter
            .acquire(&route(1))
            .await
            .complete(Outcome::Response {
                status: 200,
                headers: &RateLimitHeaders::default(),
            });

        let first = limiter.acquire(&route(1)).await;
        let second =
            tokio::time::timeout(Duration::from_millis(10), limiter.acquire(&route(1))).await;

        assert!(second.is_ok());
        drop(first);
    }

    #[tokio::test(start_paused = true)]
    async fn global_limit_spreads_requests() {
        let limiter = InMemoryRateLimiter::with_global_limit(2);
        let start = Instant::now();

        for channel in 1..=5 {
            let ticket = limiter.acquire(&route(channel)).await;
            ticket.complete(Outcome::Response {
                status: 200,
                headers: &RateLimitHeaders::default(),
            });
        }

        // 5 requests at 2 for each second need at least 2 full windows.
        assert!(start.elapsed() >= Duration::from_secs(2));
    }

    /// A very large wait time in a header made `Instant + Duration` overflow and panic.
    #[tokio::test(start_paused = true)]
    async fn very_large_wait_times_do_not_panic() {
        let parsed = {
            let mut map = http::HeaderMap::new();
            map.insert("x-ratelimit-limit", "1".parse().unwrap());
            map.insert("x-ratelimit-remaining", "0".parse().unwrap());
            map.insert(
                "x-ratelimit-reset-after",
                "18446744073709550000".parse().unwrap(),
            );
            map.insert("retry-after", "18446744073709550000".parse().unwrap());
            RateLimitHeaders::parse(&map)
        };
        assert!(parsed.reset_after.is_some());
        let global = RateLimitHeaders {
            global: true,
            ..parsed.clone()
        };

        // One route for each response, so that no acquire waits for an earlier bucket.
        let limiter = InMemoryRateLimiter::new();
        let responses = [(1, 200, &parsed), (2, 429, &parsed), (3, 429, &global)];
        for (channel, status, headers) in responses {
            limiter
                .acquire(&route(channel))
                .await
                .complete(Outcome::Response { status, headers });
        }

        assert!(limiter.global_pause_remaining().is_some());
    }
}
