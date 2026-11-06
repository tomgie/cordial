//! Authentication tokens.

use std::fmt;
use std::sync::Arc;

use zeroize::Zeroizing;

/// An authentication token: a bot token or an OAuth2 Bearer token.
///
/// `Debug` and `Display` never show the token. The memory is zeroed on drop.
#[derive(Clone)]
pub struct Token {
    /// The full value of the `Authorization` header, for example `Bot abc`.
    header: Arc<Zeroizing<String>>,
    kind: TokenKind,
}

/// The kind of a token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TokenKind {
    /// A bot token.
    Bot,
    /// An OAuth2 Bearer token.
    Bearer,
}

impl Token {
    /// Makes a bot token. A `Bot ` prefix in the text is removed first.
    #[must_use]
    pub fn bot(token: impl AsRef<str>) -> Self {
        let raw = token.as_ref().trim();
        let raw = raw.strip_prefix("Bot ").unwrap_or(raw);
        Self {
            header: Arc::new(Zeroizing::new(format!("Bot {raw}"))),
            kind: TokenKind::Bot,
        }
    }

    /// Makes an OAuth2 Bearer token. A `Bearer ` prefix in the text is removed first.
    #[must_use]
    pub fn bearer(token: impl AsRef<str>) -> Self {
        let raw = token.as_ref().trim();
        let raw = raw.strip_prefix("Bearer ").unwrap_or(raw);
        Self {
            header: Arc::new(Zeroizing::new(format!("Bearer {raw}"))),
            kind: TokenKind::Bearer,
        }
    }

    /// Reads a bot token from an environment variable.
    ///
    /// # Errors
    ///
    /// Gives an error when the variable is not set or is not valid Unicode.
    pub fn from_env(variable: &str) -> Result<Self, std::env::VarError> {
        let value = Zeroizing::new(std::env::var(variable)?);
        Ok(Self::bot(value.as_str()))
    }

    /// Gives the token kind.
    #[must_use]
    pub fn kind(&self) -> TokenKind {
        self.kind
    }

    /// Gives the value of the `Authorization` header.
    #[must_use]
    pub fn authorization(&self) -> &str {
        &self.header
    }

    /// Gives the token without the prefix, for example for the gateway Identify payload.
    #[must_use]
    pub fn secret(&self) -> &str {
        let prefix = match self.kind {
            TokenKind::Bot => "Bot ",
            TokenKind::Bearer => "Bearer ",
        };
        self.header.strip_prefix(prefix).unwrap_or(&self.header)
    }

    /// Gives the bot user ID that a bot token holds in its first part, if it can.
    ///
    /// The first part of a bot token is the base64 of the bot user ID.
    #[must_use]
    pub fn bot_user_id(&self) -> Option<u64> {
        let first = self.secret().split('.').next()?;
        let decoded = decode_base64(first)?;
        std::str::from_utf8(&decoded).ok()?.parse().ok()
    }
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    fn value(character: u8) -> Option<u8> {
        match character {
            b'A'..=b'Z' => Some(character - b'A'),
            b'a'..=b'z' => Some(character - b'a' + 26),
            b'0'..=b'9' => Some(character - b'0' + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    }

    let mut output = Vec::with_capacity(text.len() * 3 / 4);
    let mut buffer = 0_u32;
    let mut bits = 0;
    for &character in text
        .as_bytes()
        .iter()
        .filter(|&&character| character != b'=')
    {
        buffer = (buffer << 6) | u32::from(value(character)?);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            #[allow(clippy::cast_possible_truncation)]
            output.push((buffer >> bits) as u8);
        }
    }
    Some(output)
}

impl fmt::Debug for Token {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Token({:?}, <redacted>)", self.kind)
    }
}

impl fmt::Display for Token {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_secret() {
        let token = Token::bot("Bot abc.def.ghi");

        assert_eq!(token.authorization(), "Bot abc.def.ghi");
        assert_eq!(token.secret(), "abc.def.ghi");
        assert!(!format!("{token:?} {token}").contains("abc"));
    }

    #[test]
    fn reads_bot_user_id() {
        // "80351110224678912" in base64 without padding.
        let token = Token::bot("ODAzNTExMTAyMjQ2Nzg5MTI.X.Y");

        assert_eq!(token.bot_user_id(), Some(80_351_110_224_678_912));
    }
}
