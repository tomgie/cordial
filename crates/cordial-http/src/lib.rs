//! HTTP client for the Discord API.
//!
//! This crate contains one request type for each endpoint, the rate limiter, and OAuth2.
//! See `docs/ARCHITECTURE.md`, section 6.
//!
//! ```no_run
//! # async fn run() -> Result<(), cordial_http::Error> {
//! use cordial_http::{Client, Token};
//! use cordial_model::id::ChannelId;
//!
//! let client = Client::new(Token::bot("TOKEN"));
//! let message = client
//!     .create_message(ChannelId::new_const(1234))
//!     .content("Hello")
//!     .await?;
//! # Ok(())
//! # }
//! ```

mod client;
pub mod endpoints;
mod error;
pub mod json_error;
mod payload;
pub mod ratelimit;
pub mod request;
pub mod route;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod token;
pub mod transport;

pub use client::{API_BASE, Client, ClientBuilder};
pub use error::{ApiError, Error, FieldError, TransportError};
pub use json_error::JsonErrorCode;
pub use payload::MessagePayload;
pub use token::{Token, TokenKind};

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC};

/// The characters that a query value or a header value must encode.
const QUERY_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// The characters that a path segment must encode.
const PATH_SEGMENT_ENCODE_SET: &AsciiSet = QUERY_ENCODE_SET;
