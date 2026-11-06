//! Entry points into the DAVE frame codec for fuzz targets.
//!
//! The frame parser and the per-sender cryptors are private. This module gives fuzz targets
//! access to them with a fixed key. It is not part of the public API and can change in any
//! release.

use std::time::Instant;

use super::decryptor::Decryptor;
use super::encryptor::Encryptor;
use super::frame;
use super::ratchet::KeyRatchet;

/// The parts of a parsed protocol frame.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct FrameParts {
    /// The interleaved media frame, without the footer.
    pub media: Vec<u8>,
    /// The unencrypted ranges joined.
    pub authenticated: Vec<u8>,
    /// The encrypted bytes joined.
    pub ciphertext: Vec<u8>,
    /// The media frame that the reconstruction makes from `ciphertext`. It must equal `media`.
    pub rebuilt: Vec<u8>,
    /// The truncated nonce.
    pub nonce: u32,
}

/// Parses a protocol frame, splits it, and puts it together again.
///
/// Gives `None` when the frame is not a valid protocol frame.
#[must_use]
pub fn parse_frame(protocol_frame: &[u8]) -> Option<FrameParts> {
    let parsed = frame::parse(protocol_frame)?;
    let (authenticated, ciphertext) = parsed.split();
    let rebuilt = parsed.reconstruct(&ciphertext);
    Some(FrameParts {
        media: parsed.media.to_vec(),
        authenticated,
        ciphertext,
        rebuilt,
        nonce: parsed.nonce,
    })
}

/// A sender and a receiver that share one fixed key ratchet.
#[derive(Debug)]
pub struct FixedKeyCodec {
    encryptor: Encryptor,
    decryptor: Decryptor,
}

impl FixedKeyCodec {
    /// Makes a codec from a base secret.
    #[must_use]
    pub fn new(base_secret: &[u8]) -> Self {
        let mut encryptor = Encryptor::new();
        encryptor.set_key_ratchet(Some(KeyRatchet::new(base_secret.to_vec())));
        encryptor.set_passthrough(false);

        let mut decryptor = Decryptor::new(false);
        decryptor
            .transition_to_key_ratchet(Some(KeyRatchet::new(base_secret.to_vec())), Instant::now());
        Self {
            encryptor,
            decryptor,
        }
    }

    /// Encrypts an Opus frame. Gives `None` when the encryption fails.
    pub fn encrypt(&mut self, opus_frame: &[u8]) -> Option<Vec<u8>> {
        self.encryptor.encrypt(opus_frame).ok()
    }

    /// Decrypts a protocol frame. Gives `None` when the frame does not decrypt.
    pub fn decrypt(&mut self, protocol_frame: &[u8]) -> Option<Vec<u8>> {
        self.decryptor.decrypt(protocol_frame, Instant::now()).ok()
    }
}
