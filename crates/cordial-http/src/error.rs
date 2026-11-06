//! Errors of the HTTP client.

use std::fmt;
use std::time::Duration;

use cordial_model::validate::ValidationError;
use serde::Deserialize;
use serde_json::Value;

use crate::json_error::JsonErrorCode;
use crate::route::Method;

/// An error of an HTTP request.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Discord answered with an error status.
    #[error(transparent)]
    Api(Box<ApiError>),
    /// A value breaks a documented limit. The request was not sent.
    #[error("the request is not valid: {0}")]
    Validation(#[from] ValidationError),
    /// The request was rate limited, and the client did not wait (or waited too often).
    #[error("rate limited on {route}: retry after {retry_after:?}")]
    RateLimited {
        /// The route template.
        route: &'static str,
        /// The time to wait.
        retry_after: Duration,
        /// Tells if the limit is the global limit.
        global: bool,
    },
    /// An earlier request got status 401. The client stops all requests with this token.
    #[error("the token is not valid (an earlier request got status 401)")]
    InvalidToken,
    /// The request needs a token, but the client has none.
    #[error("the request needs a token, but the client has none")]
    MissingToken,
    /// The connection failed or timed out.
    #[error("the transport failed: {0}")]
    Transport(#[from] TransportError),
    /// The response body does not match the expected type.
    #[error("cannot decode the response of {method} {route}: {source}")]
    Decode {
        /// The method.
        method: Method,
        /// The route template.
        route: &'static str,
        /// The decode error.
        #[source]
        source: serde_json::Error,
        /// The start of the body, for diagnosis.
        body_start: String,
    },
    /// A request body could not be encoded.
    #[error("cannot encode the request body: {0}")]
    Encode(#[source] serde_json::Error),
}

impl Error {
    /// Gives the API error, if Discord answered with an error status.
    #[must_use]
    pub fn api(&self) -> Option<&ApiError> {
        match self {
            Self::Api(error) => Some(error),
            _ => None,
        }
    }

    /// Gives the JSON error code, if Discord sent one.
    #[must_use]
    pub fn code(&self) -> Option<JsonErrorCode> {
        self.api().and_then(|error| error.code)
    }

    /// Gives the HTTP status, if Discord answered.
    #[must_use]
    pub fn status(&self) -> Option<u16> {
        self.api().map(|error| error.status)
    }

    /// Tells if the error is "not found" (status 404).
    #[must_use]
    pub fn is_not_found(&self) -> bool {
        self.status() == Some(404)
    }
}

/// An error response of the Discord API.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ApiError {
    /// The HTTP status.
    pub status: u16,
    /// The method of the request.
    pub method: Method,
    /// The route template of the request.
    pub route: &'static str,
    /// The JSON error code.
    pub code: Option<JsonErrorCode>,
    /// The message.
    pub message: String,
    /// The tree of field errors, as Discord sends it.
    pub errors: Option<Value>,
    /// For errors that carry a wait time (for example search index errors).
    pub retry_after: Option<Duration>,
}

impl ApiError {
    /// Gives the field errors as a flat list: (path, code, message).
    ///
    /// Example: `("embeds.0.title", "BASE_TYPE_MAX_LENGTH", "Must be 256 or fewer in length.")`.
    #[must_use]
    pub fn field_errors(&self) -> Vec<FieldError> {
        let mut found = Vec::new();
        if let Some(errors) = &self.errors {
            collect_field_errors(errors, "", &mut found);
        }
        found
    }

    pub(crate) fn from_response(
        status: u16,
        method: Method,
        route: &'static str,
        body: &[u8],
    ) -> Self {
        #[derive(Deserialize)]
        struct Body {
            #[serde(default)]
            code: Option<u32>,
            #[serde(default)]
            message: Option<String>,
            #[serde(default)]
            errors: Option<Value>,
            #[serde(default)]
            retry_after: Option<f64>,
        }

        let parsed: Option<Body> = serde_json::from_slice(body).ok();
        let message = parsed
            .as_ref()
            .and_then(|body| body.message.clone())
            .unwrap_or_else(|| String::from_utf8_lossy(&body[..body.len().min(200)]).into_owned());

        Self {
            status,
            method,
            route,
            code: parsed
                .as_ref()
                .and_then(|body| body.code)
                .map(JsonErrorCode::from_value),
            message,
            errors: parsed.as_ref().and_then(|body| body.errors.clone()),
            retry_after: parsed
                .and_then(|body| body.retry_after)
                .and_then(|seconds| Duration::try_from_secs_f64(seconds).ok()),
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} {} returned {}",
            self.method, self.route, self.status
        )?;
        if let Some(code) = self.code {
            write!(formatter, " (code {})", code.value())?;
        }
        write!(formatter, ": {}", self.message)?;
        for field in self.field_errors() {
            write!(formatter, "; {}: {}", field.path, field.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

/// One field error of an API error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FieldError {
    /// The path of the field, for example `embeds.0.title`.
    pub path: String,
    /// The error code, for example `BASE_TYPE_REQUIRED`.
    pub code: String,
    /// The message.
    pub message: String,
}

fn collect_field_errors(value: &Value, path: &str, found: &mut Vec<FieldError>) {
    let Value::Object(map) = value else {
        return;
    };

    for (key, child) in map {
        if key == "_errors" {
            for error in child.as_array().into_iter().flatten() {
                found.push(FieldError {
                    path: path.to_owned(),
                    code: error["code"].as_str().unwrap_or_default().to_owned(),
                    message: error["message"].as_str().unwrap_or_default().to_owned(),
                });
            }
        } else {
            let child_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            collect_field_errors(child, &child_path, found);
        }
    }
}

/// An error of the transport (the connection), not of Discord.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TransportError {
    /// The request did not complete in time.
    #[error("the request timed out after {0:?}")]
    Timeout(Duration),
    /// The connection failed.
    #[error("the connection failed: {0}")]
    Connection(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// The request could not be built.
    #[error("the request is not valid: {0}")]
    InvalidRequest(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_error_body() {
        let body = br#"{"code":50035,"errors":{"embeds":{"0":{"title":{"_errors":[{"code":"BASE_TYPE_MAX_LENGTH","message":"Too long."}]}}}},"message":"Invalid Form Body"}"#;

        let error =
            ApiError::from_response(400, Method::Post, "/channels/{channel_id}/messages", body);

        assert_eq!(error.code, Some(JsonErrorCode::InvalidFormBody));
        let fields = error.field_errors();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].path, "embeds.0.title");
        assert!(error.to_string().contains("embeds.0.title: Too long."));
    }

    #[test]
    fn keeps_non_json_body() {
        let error =
            ApiError::from_response(502, Method::Get, "/gateway", b"<html>Bad Gateway</html>");

        assert_eq!(error.code, None);
        assert!(error.message.contains("Bad Gateway"));
    }
}
