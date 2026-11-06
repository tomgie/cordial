//! Ed25519 signature checks of requests from Discord.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ed25519_dalek::{Signature, VerifyingKey};

/// The header with the hex signature.
pub const SIGNATURE_HEADER: &str = "X-Signature-Ed25519";
/// The header with the timestamp.
pub const TIMESTAMP_HEADER: &str = "X-Signature-Timestamp";

/// The public key of an application (the "Public Key" in the Developer Portal).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublicKey(VerifyingKey);

impl PublicKey {
    /// Reads a public key from its hex text.
    ///
    /// # Errors
    ///
    /// Gives an error when the text is not 64 hex digits or not a valid key.
    pub fn from_hex(text: &str) -> Result<Self, VerifyError> {
        let bytes: [u8; 32] = decode_hex(text.trim())
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(VerifyError::InvalidKey)?;
        VerifyingKey::from_bytes(&bytes)
            .map(Self)
            .map_err(|_| VerifyError::InvalidKey)
    }

    /// Makes a public key from its 32 bytes.
    ///
    /// # Errors
    ///
    /// Gives an error when the bytes are not a valid key.
    pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, VerifyError> {
        VerifyingKey::from_bytes(bytes)
            .map(Self)
            .map_err(|_| VerifyError::InvalidKey)
    }

    /// Checks the signature of a request.
    ///
    /// Discord signs the timestamp text followed by the raw body. Use the body bytes exactly
    /// as they arrived: do not decode and encode them again.
    ///
    /// # Errors
    ///
    /// Gives an error when a header is missing or the signature does not match.
    pub fn verify(
        &self,
        signature: Option<&str>,
        timestamp: Option<&str>,
        body: &[u8],
    ) -> Result<(), VerifyError> {
        let signature = signature.ok_or(VerifyError::MissingSignature)?;
        let timestamp = timestamp.ok_or(VerifyError::MissingTimestamp)?;
        let signature: [u8; 64] = decode_hex(signature.trim())
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(VerifyError::BadSignature)?;
        let signature = Signature::from_bytes(&signature);

        let mut message = Vec::with_capacity(timestamp.len() + body.len());
        message.extend_from_slice(timestamp.as_bytes());
        message.extend_from_slice(body);

        self.0
            .verify_strict(&message, &signature)
            .map_err(|_| VerifyError::BadSignature)
    }
}

/// Checks that a timestamp (Unix seconds) is at most `max_age` away from the current time.
///
/// # Errors
///
/// Gives an error when the timestamp is not a number or is too old or too far in the future.
pub fn check_timestamp(timestamp: &str, max_age: Duration) -> Result<(), VerifyError> {
    let seconds: u64 = timestamp
        .trim()
        .parse()
        .map_err(|_| VerifyError::StaleTimestamp)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    if now.abs_diff(seconds) > max_age.as_secs() {
        return Err(VerifyError::StaleTimestamp);
    }
    Ok(())
}

/// An error of a signature check.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum VerifyError {
    /// The public key is not valid.
    #[error("the public key is not valid (it must be 64 hex digits)")]
    InvalidKey,
    /// The signature header is missing.
    #[error("the {SIGNATURE_HEADER} header is missing")]
    MissingSignature,
    /// The timestamp header is missing.
    #[error("the {TIMESTAMP_HEADER} header is missing")]
    MissingTimestamp,
    /// The signature does not match.
    #[error("the signature does not match")]
    BadSignature,
    /// The timestamp is too old or too far in the future.
    #[error("the timestamp is too old or too far in the future")]
    StaleTimestamp,
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let high = char::from(pair[0]).to_digit(16)?;
            let low = char::from(pair[1]).to_digit(16)?;
            u8::try_from(high * 16 + low).ok()
        })
        .collect()
}
