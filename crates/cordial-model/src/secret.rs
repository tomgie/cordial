//! Secret text, such as a token, that must not appear in logs.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Secret text, for example an interaction token or a webhook token.
///
/// `Debug` does not show the text, so that a log of a payload does not leak it. Use
/// [`Secret::expose`] to read the text.
#[derive(Clone, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    /// Makes a secret.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// Gives the secret text.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Secret(<redacted>)")
    }
}

impl From<String> for Secret {
    fn from(text: String) -> Self {
        Self(text)
    }
}

impl From<&str> for Secret {
    fn from(text: &str) -> Self {
        Self(text.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_text() {
        let secret = Secret::new("abc");

        assert_eq!(format!("{secret:?}"), "Secret(<redacted>)");
        assert_eq!(secret.expose(), "abc");
        assert_eq!(serde_json::to_string(&secret).unwrap(), "\"abc\"");
    }
}
