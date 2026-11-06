//! Routes: the method, the path, and the rate limit identity of a request.

use std::fmt;

use cordial_model::id::{ChannelId, GuildId, WebhookId};

/// An HTTP method.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Method {
    /// GET.
    Get,
    /// POST.
    Post,
    /// PUT.
    Put,
    /// PATCH.
    Patch,
    /// DELETE.
    Delete,
}

impl Method {
    /// Gives the method name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
        }
    }

    /// Tells if a repeated request has the same effect as one request.
    #[must_use]
    pub const fn is_idempotent(self) -> bool {
        !matches!(self, Self::Post)
    }
}

impl fmt::Display for Method {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<Method> for http::Method {
    fn from(method: Method) -> Self {
        match method {
            Method::Get => Self::GET,
            Method::Post => Self::POST,
            Method::Put => Self::PUT,
            Method::Patch => Self::PATCH,
            Method::Delete => Self::DELETE,
        }
    }
}

/// The top-level ("major") parameter of a route. Discord counts rate limits separately for
/// each value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MajorParameter {
    /// The route has no major parameter.
    None,
    /// A channel.
    Channel(ChannelId),
    /// A guild.
    Guild(GuildId),
    /// A webhook, with a hash of its token.
    Webhook(WebhookId, u64),
}

/// How a route authenticates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Authentication {
    /// The client token (the default).
    Token,
    /// No `Authorization` header. Routes with a webhook or interaction token in the path.
    None,
}

/// The method, the path, and the rate limit identity of a request.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Route {
    /// The method.
    pub method: Method,
    /// The path template, for example `/channels/{channel_id}/messages`. Rate limits use it.
    pub template: &'static str,
    /// The path with values and the query string, for example `/channels/1/messages?limit=5`.
    pub path: String,
    /// The major parameter.
    pub major: MajorParameter,
    /// Tells if the request counts against the global rate limit.
    pub global_limit: bool,
    /// How the request authenticates.
    pub authentication: Authentication,
}

impl Route {
    /// Makes a route that uses the client token and the global rate limit.
    #[must_use]
    pub fn new(
        method: Method,
        template: &'static str,
        path: String,
        major: MajorParameter,
    ) -> Self {
        Self {
            method,
            template,
            path,
            major,
            global_limit: true,
            authentication: Authentication::Token,
        }
    }

    /// Marks the route as one that sends no `Authorization` header and is outside the global
    /// rate limit (webhook and interaction token routes).
    #[must_use]
    pub fn with_path_token(mut self) -> Self {
        self.global_limit = false;
        self.authentication = Authentication::None;
        self
    }

    /// Gives the key that identifies the rate limit of the route.
    #[must_use]
    pub fn key(&self) -> RouteKey {
        RouteKey {
            method: self.method,
            template: self.template,
            major: self.major,
        }
    }
}

/// The identity of a route for rate limits: the method, the template, and the major parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct RouteKey {
    /// The method.
    pub method: Method,
    /// The path template.
    pub template: &'static str,
    /// The major parameter.
    pub major: MajorParameter,
}

/// Gives a short hash of a webhook token, so that the rate limit key does not hold the secret.
#[must_use]
pub fn token_hash(token: &str) -> u64 {
    // FNV-1a: stable and fast. It needs no security; it only tells tokens apart.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in token.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Builds a query string.
#[derive(Debug, Default)]
pub struct Query {
    text: String,
}

impl Query {
    /// Makes an empty query.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a parameter. The value is percent-encoded.
    pub fn push(&mut self, name: &str, value: impl fmt::Display) {
        let separator = if self.text.is_empty() { '?' } else { '&' };
        let value = value.to_string();
        let encoded = percent_encoding::utf8_percent_encode(&value, crate::QUERY_ENCODE_SET);
        self.text.push(separator);
        self.text.push_str(name);
        self.text.push('=');
        self.text.push_str(&encoded.to_string());
    }

    /// Adds a parameter when the value is set.
    pub fn push_opt(&mut self, name: &str, value: Option<impl fmt::Display>) {
        if let Some(value) = value {
            self.push(name, value);
        }
    }

    /// Gives the query string, with `?` when it is not empty.
    #[must_use]
    pub fn finish(self) -> String {
        self.text
    }
}

/// Percent-encodes one path segment, for example an emoji in a reaction route.
#[must_use]
pub fn encode_segment(segment: &str) -> String {
    percent_encoding::utf8_percent_encode(segment, crate::PATH_SEGMENT_ENCODE_SET).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_query() {
        let mut query = Query::new();
        query.push("limit", 50);
        query.push_opt("before", None::<u64>);
        query.push("q", "a b&c");

        assert_eq!(query.finish(), "?limit=50&q=a%20b%26c");
    }

    #[test]
    fn encodes_emoji_segment() {
        assert_eq!(encode_segment("👍"), "%F0%9F%91%8D");
        assert_eq!(encode_segment("name:123"), "name%3A123");
    }
}
