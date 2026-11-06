//! HTTP responses: the error body, the rate-limit headers, and large model types.

use std::time::Duration;

use bytes::Bytes;
use cordial_http::ratelimit::{InMemoryRateLimiter, Outcome, RateLimitHeaders, RateLimiter};
use cordial_http::route::{MajorParameter, Method, Route};
use cordial_http::testing::{FakeResponse, FakeTransport};
use cordial_http::{Client, Token};
use cordial_model::channel::Channel;
use cordial_model::guild::Guild;
use cordial_model::id::ChannelId;
use cordial_model::interaction::Interaction;
use cordial_model::message::Message;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::block_on;
use crate::reader::Reader;

const STATUSES: [u16; 12] = [200, 201, 204, 400, 401, 403, 404, 405, 429, 500, 502, 503];

/// The headers that the body fills, in order: one line of the input for each.
const HEADER_NAMES: [&str; 7] = [
    "x-ratelimit-limit",
    "x-ratelimit-remaining",
    "x-ratelimit-reset-after",
    "x-ratelimit-bucket",
    "x-ratelimit-global",
    "x-ratelimit-scope",
    "retry-after",
];

const CHANNEL: ChannelId = ChannelId::new_const(1);

/// An HTTP response.
///
/// The selector byte chooses the status. The next lines are the values of [`HEADER_NAMES`]
/// (an empty line leaves a header out). The rest is the body.
pub fn http_response(data: &[u8]) {
    let mut reader = Reader::new(data);
    let status = STATUSES[usize::from(reader.byte()) % STATUSES.len()];
    let (headers, body) = split_headers(reader.rest());

    decode_models(body);
    rate_limit(status, &headers);
    client_response(status, &headers, body);
}

fn split_headers(mut rest: &[u8]) -> (Vec<(&'static str, String)>, &[u8]) {
    let mut headers = Vec::new();
    for name in HEADER_NAMES {
        let Some(end) = rest.iter().position(|byte| *byte == b'\n') else {
            break;
        };
        let value = &rest[..end];
        rest = &rest[end + 1..];
        if value.is_empty() {
            continue;
        }

        // Only visible ASCII is valid in a header value. Other bytes become spaces.
        let text = value
            .iter()
            .map(|&byte| {
                if byte.is_ascii_graphic() {
                    char::from(byte)
                } else {
                    ' '
                }
            })
            .collect();
        headers.push((name, text));
    }
    (headers, rest)
}

fn decode_models(body: &[u8]) {
    decode::<Message>(body);
    decode::<Guild>(body);
    decode::<Interaction>(body);
    decode::<Channel>(body);
}

/// Decodes a value. When it decodes, its encoding must decode again.
fn decode<T: Serialize + DeserializeOwned>(body: &[u8]) {
    let Ok(value) = serde_json::from_slice::<T>(body) else {
        return;
    };
    let encoded = serde_json::to_vec(&value).expect("a decoded value encodes");
    if let Err(error) = serde_json::from_slice::<T>(&encoded) {
        panic!(
            "the encoding of a decoded {} does not decode: {error}\n{}",
            std::any::type_name::<T>(),
            String::from_utf8_lossy(&encoded)
        );
    }
}

fn header_map(headers: &[(&'static str, String)]) -> http::HeaderMap {
    let mut map = http::HeaderMap::new();
    for (name, value) in headers {
        if let Ok(value) = http::HeaderValue::from_str(value) {
            map.append(*name, value);
        }
    }
    map
}

fn rate_limit(status: u16, headers: &[(&'static str, String)]) {
    let parsed = RateLimitHeaders::parse(&header_map(headers));
    let route = Route::new(
        Method::Post,
        "/channels/{channel_id}/messages",
        format!("/channels/{CHANNEL}/messages"),
        MajorParameter::Channel(CHANNEL),
    );
    let limiter = InMemoryRateLimiter::new();

    block_on(async {
        let ticket = limiter.acquire(&route).await;
        ticket.complete(Outcome::Response {
            status,
            headers: &parsed,
        });

        // The next request waits or goes out. The time is paused, so a long wait ends at once.
        let next = limiter.acquire(&route);
        if let Ok(ticket) = tokio::time::timeout(Duration::from_secs(3600), next).await {
            ticket.complete(Outcome::Failed);
        }
        let _ = limiter.global_pause_remaining();
    });
}

fn client_response(status: u16, headers: &[(&'static str, String)], body: &[u8]) {
    let transport = FakeTransport::new();
    let mut response = FakeResponse::empty(status);
    for (name, value) in headers {
        response = response.header(name, value);
    }
    response.body = Bytes::copy_from_slice(body);
    transport.push(response);

    let client = Client::builder()
        .token(Token::bot("token"))
        .transport(transport)
        .max_retries(0)
        .max_rate_limit_retries(0)
        .build();

    block_on(async {
        match client.channel(CHANNEL).await {
            Ok(channel) => {
                let _ = serde_json::to_vec(&channel);
            }
            Err(error) => {
                let _ = error.to_string();
                if let Some(api) = error.api() {
                    let _ = api.field_errors();
                }
            }
        }
    });
}
