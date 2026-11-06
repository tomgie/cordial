//! The overhead of the rate limiter for each request: acquire a ticket, then complete it with
//! the rate-limit headers of the response.

use std::hint::black_box;
use std::time::Duration;

use cordial_http::ratelimit::{
    InMemoryRateLimiter, NoRateLimiter, Outcome, RateLimitHeaders, RateLimiter,
};
use cordial_http::route::{MajorParameter, Method, Route};
use cordial_model::id::ChannelId;
use criterion::{Criterion, criterion_group, criterion_main};

const CHANNEL: ChannelId = ChannelId::new_const(1_300_000_000_000_000_000);

fn route() -> Route {
    Route::new(
        Method::Post,
        "/channels/{channel_id}/messages",
        format!("/channels/{CHANNEL}/messages"),
        MajorParameter::Channel(CHANNEL),
    )
}

/// Headers of a bucket that never runs out, so that no request waits.
fn headers() -> RateLimitHeaders {
    let mut map = http::HeaderMap::new();
    map.insert("x-ratelimit-limit", http::HeaderValue::from(u32::MAX));
    map.insert("x-ratelimit-remaining", http::HeaderValue::from(u32::MAX));
    map.insert(
        "x-ratelimit-reset-after",
        http::HeaderValue::from_static("1.000"),
    );
    map.insert(
        "x-ratelimit-bucket",
        http::HeaderValue::from_static("4b2c9a1e5d"),
    );
    RateLimitHeaders::parse(&map)
}

fn one_request(
    runtime: &tokio::runtime::Runtime,
    limiter: &dyn RateLimiter,
    route: &Route,
    headers: &RateLimitHeaders,
) {
    runtime.block_on(async {
        let ticket = limiter.acquire(black_box(route)).await;
        ticket.complete(Outcome::Response {
            status: 200,
            headers: black_box(headers),
        });
    });
}

fn rate_limiter(criterion: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("a Tokio runtime");
    let route = route();
    let headers = headers();
    assert!(headers.has_bucket_data());

    let mut group = criterion.benchmark_group("rate_limiter");
    group.measurement_time(Duration::from_secs(3));

    // The global limit (50 for each second by default) would make the loop wait. A very high
    // limit keeps its check in the measurement but never waits.
    let in_memory = InMemoryRateLimiter::with_global_limit(u32::MAX);
    group.bench_function("in_memory_acquire_complete", |bencher| {
        bencher.iter(|| one_request(&runtime, &in_memory, &route, &headers));
    });
    let no_limiter = NoRateLimiter::default();
    group.bench_function("no_limiter_acquire_complete", |bencher| {
        bencher.iter(|| one_request(&runtime, &no_limiter, &route, &headers));
    });

    let mut map = http::HeaderMap::new();
    map.insert("x-ratelimit-limit", http::HeaderValue::from_static("5"));
    map.insert("x-ratelimit-remaining", http::HeaderValue::from_static("4"));
    map.insert(
        "x-ratelimit-reset-after",
        http::HeaderValue::from_static("1.250"),
    );
    map.insert(
        "x-ratelimit-bucket",
        http::HeaderValue::from_static("4b2c9a1e5d"),
    );
    group.bench_function("parse_headers", |bencher| {
        bencher.iter(|| RateLimitHeaders::parse(black_box(&map)));
    });
    group.finish();
}

criterion_group!(benches, rate_limiter);
criterion_main!(benches);
