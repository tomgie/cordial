//! The transport: the part that sends HTTP requests over the network.
//!
//! The client uses the [`Transport`] trait. Tests use a fake transport, and other runtimes or
//! platforms can give their own.

use std::fmt;
use std::future::Future;
use std::pin::Pin;

use bytes::Bytes;

use crate::error::TransportError;

/// A boxed future that is `Send`.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Sends HTTP requests.
pub trait Transport: Send + Sync + fmt::Debug + 'static {
    /// Sends a request and gives the response.
    fn send(
        &self,
        request: http::Request<Bytes>,
    ) -> BoxFuture<'static, Result<http::Response<Bytes>, TransportError>>;
}

/// The default transport: `reqwest`.
#[cfg(feature = "reqwest-transport")]
#[derive(Clone, Debug)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

#[cfg(feature = "reqwest-transport")]
impl ReqwestTransport {
    /// Makes a transport with a new `reqwest` client.
    ///
    /// # Panics
    ///
    /// Panics when the TLS backend cannot start, as `reqwest::Client::new` does.
    #[must_use]
    pub fn new() -> Self {
        Self::with_client(reqwest::Client::new())
    }

    /// Makes a transport with an existing `reqwest` client, for example with a proxy.
    #[must_use]
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[cfg(feature = "reqwest-transport")]
impl Default for ReqwestTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "reqwest-transport")]
impl Transport for ReqwestTransport {
    fn send(
        &self,
        request: http::Request<Bytes>,
    ) -> BoxFuture<'static, Result<http::Response<Bytes>, TransportError>> {
        let client = self.client.clone();
        Box::pin(async move {
            let request = reqwest::Request::try_from(request.map(reqwest::Body::from))
                .map_err(|error| TransportError::InvalidRequest(error.to_string()))?;
            let response = client
                .execute(request)
                .await
                .map_err(|error| TransportError::Connection(Box::new(error)))?;

            let status = response.status();
            let headers = response.headers().clone();
            let body = response
                .bytes()
                .await
                .map_err(|error| TransportError::Connection(Box::new(error)))?;

            let mut converted = http::Response::new(body);
            *converted.status_mut() = status;
            *converted.headers_mut() = headers;
            Ok(converted)
        })
    }
}
