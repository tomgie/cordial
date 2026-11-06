//! Helpers for the endpoint tests.

#![allow(dead_code)]

use cordial_http::testing::{FakeResponse, FakeTransport, RecordedRequest};
use cordial_http::{Client, Token};
use serde_json::json;

/// The base URL of the API in the recorded requests.
pub(crate) const BASE: &str = "https://discord.com/api/v10";

/// Makes a client with a bot token and a fake transport.
pub(crate) fn bot_client(transport: &FakeTransport) -> Client {
    Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .build()
}

/// Makes a client with a Bearer token and a fake transport.
pub(crate) fn bearer_client(transport: &FakeTransport) -> Client {
    Client::builder()
        .token(Token::bearer("user-token"))
        .transport(transport.clone())
        .build()
}

/// Makes a client without a token and a fake transport.
pub(crate) fn tokenless_client(transport: &FakeTransport) -> Client {
    Client::builder().transport(transport.clone()).build()
}

/// Makes a fake transport with one JSON response.
pub(crate) fn respond(status: u16, body: &serde_json::Value) -> FakeTransport {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::json(status, body));
    transport
}

/// Makes a fake transport with one "204 No Content" response.
pub(crate) fn respond_empty() -> FakeTransport {
    let transport = FakeTransport::new();
    transport.push(FakeResponse::empty(204));
    transport
}

/// Gives the only request that the transport received.
pub(crate) fn only_request(transport: &FakeTransport) -> RecordedRequest {
    let requests = transport.requests();
    assert_eq!(requests.len(), 1, "expected one request");
    requests.into_iter().next().unwrap()
}

/// Checks the method, the path with the query, and the `Authorization` header.
pub(crate) fn assert_request(
    request: &RecordedRequest,
    method: &str,
    path: &str,
    authorization: Option<&str>,
) {
    assert_eq!(request.method, method);
    assert_eq!(request.url, format!("{BASE}{path}"));
    assert_eq!(request.header("authorization"), authorization);
}

/// Checks that the request has no body.
pub(crate) fn assert_no_body(request: &RecordedRequest) {
    assert!(request.body.is_empty(), "expected no body");
    assert_eq!(request.header("content-type"), None);
}

/// Checks that the request has a JSON body with this value.
pub(crate) fn assert_json(request: &RecordedRequest, expected: &serde_json::Value) {
    assert_eq!(request.header("content-type"), Some("application/json"));
    assert_eq!(&request.json(), expected);
}

/// Decodes a form body into name and value pairs.
pub(crate) fn form(request: &RecordedRequest) -> Vec<(String, String)> {
    assert_eq!(
        request.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
    let text = std::str::from_utf8(&request.body).unwrap();
    text.split('&')
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap();
            let value = percent_encoding::percent_decode_str(value)
                .decode_utf8()
                .unwrap()
                .into_owned();
            (name.to_owned(), value)
        })
        .collect()
}

/// A user, as in the docs.
pub(crate) fn user_json() -> serde_json::Value {
    json!({
        "id": "80351110224678912",
        "username": "Nelly",
        "discriminator": "1337",
        "global_name": null,
        "avatar": "8342729096ea3675442027381ff50dfe",
        "verified": true,
        "email": "nelly@discord.com",
        "flags": 64,
        "banner": "06c16474723fe537c283b8efa61a30c8",
        "accent_color": 16711680,
        "premium_type": 1,
        "public_flags": 64
    })
}
