//! The send side: the encryptor of the own user.
//!
//! The logic follows libdave `encryptor.cpp`. The truncated nonce increments before each frame,
//! so the first frame of a key domain has the nonce 1. The most significant byte of the nonce
//! selects the key generation.

use super::cryptor::Cryptor;
use super::decryptor::{GENERATION_SHIFT, wrapped_generation};
use super::frame;
use super::ratchet::KeyRatchet;

/// Why a frame does not encrypt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EncryptFailure {
    /// No key ratchet is available.
    MissingKey,
    /// The cipher failed.
    Cipher,
}

impl EncryptFailure {
    /// Gives a short description.
    pub(crate) fn describe(self) -> &'static str {
        match self {
            Self::MissingKey => "no key is available for the own user",
            Self::Cipher => "the cipher failed",
        }
    }
}

/// The encryptor of the own user.
#[derive(Debug)]
pub(crate) struct Encryptor {
    ratchet: Option<KeyRatchet>,
    domain: Vec<u8>,
    generation: u32,
    nonce: u32,
    cryptor: Option<Cryptor>,
    passthrough: bool,
}

impl Encryptor {
    /// Makes an encryptor in passthrough mode.
    pub(crate) fn new() -> Self {
        Self {
            ratchet: None,
            domain: Vec::new(),
            generation: 0,
            nonce: 0,
            cryptor: None,
            passthrough: true,
        }
    }

    /// Sets the key ratchet. Only a new key domain restarts the nonce sequence, because a
    /// restart in the same domain reuses nonces (libdave `Encryptor::SetKeyRatchet`).
    pub(crate) fn set_key_ratchet(&mut self, ratchet: Option<KeyRatchet>) {
        if let Some(ratchet) = &ratchet
            && ratchet.domain() != self.domain
        {
            self.domain = ratchet.domain().to_vec();
            self.generation = 0;
            self.nonce = 0;
        }
        self.ratchet = ratchet;
        self.cryptor = None;
    }

    /// Sets whether frames pass through without encryption.
    pub(crate) fn set_passthrough(&mut self, passthrough: bool) {
        self.passthrough = passthrough;
    }

    /// Encrypts an Opus frame. All bytes of an Opus frame are encrypted and no authenticated
    /// data is used.
    pub(crate) fn encrypt(&mut self, opus_frame: &[u8]) -> Result<Vec<u8>, EncryptFailure> {
        if self.passthrough {
            return Ok(opus_frame.to_vec());
        }

        let nonce = self.next_nonce()?;
        let cryptor = self.cryptor.as_ref().ok_or(EncryptFailure::MissingKey)?;
        let (ciphertext, tag) = cryptor
            .encrypt(nonce, &[], opus_frame)
            .ok_or(EncryptFailure::Cipher)?;
        Ok(frame::encode_full(&ciphertext, &tag, nonce))
    }

    fn next_nonce(&mut self) -> Result<u32, EncryptFailure> {
        let ratchet = self.ratchet.as_mut().ok_or(EncryptFailure::MissingKey)?;

        let nonce = self.nonce.wrapping_add(1);
        let generation = wrapped_generation(self.generation, nonce >> GENERATION_SHIFT);
        if generation != self.generation || self.cryptor.is_none() {
            let key = ratchet.get(generation).ok_or(EncryptFailure::MissingKey)?;
            if generation != self.generation {
                ratchet.erase(self.generation);
            }
            self.generation = generation;
            self.cryptor = Some(Cryptor::new(&key));
        }

        self.nonce = nonce;
        Ok(nonce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dave::frame::parse;

    #[test]
    fn passthrough_and_missing_key() {
        let mut encryptor = Encryptor::new();
        assert_eq!(encryptor.encrypt(b"abc").unwrap(), b"abc");
        encryptor.set_passthrough(false);
        assert_eq!(encryptor.encrypt(b"abc"), Err(EncryptFailure::MissingKey));
    }

    #[test]
    fn nonce_increments_and_restarts_only_for_new_domain() {
        let mut encryptor = Encryptor::new();
        encryptor.set_passthrough(false);
        encryptor.set_key_ratchet(Some(KeyRatchet::new(vec![1; 16])));

        let nonce_of = |frame: Vec<u8>| parse(&frame).unwrap().nonce;
        assert_eq!(nonce_of(encryptor.encrypt(b"a").unwrap()), 1);
        assert_eq!(nonce_of(encryptor.encrypt(b"b").unwrap()), 2);

        encryptor.set_key_ratchet(Some(KeyRatchet::new(vec![1; 16])));
        assert_eq!(nonce_of(encryptor.encrypt(b"c").unwrap()), 3);

        encryptor.set_key_ratchet(Some(KeyRatchet::new(vec![2; 16])));
        assert_eq!(nonce_of(encryptor.encrypt(b"d").unwrap()), 1);
    }

    #[test]
    fn generation_follows_nonce_high_byte() {
        let mut encryptor = Encryptor::new();
        encryptor.set_passthrough(false);
        encryptor.set_key_ratchet(Some(KeyRatchet::new(vec![3; 16])));
        encryptor.nonce = 0x00FF_FFFF;

        let frame = encryptor.encrypt(b"x").unwrap();
        assert_eq!(parse(&frame).unwrap().nonce, 0x0100_0000);
        assert_eq!(encryptor.generation, 1);

        // The nonce wraps after 2^32 frames and the generation keeps its count.
        encryptor.nonce = u32::MAX;
        encryptor.generation = 255;
        let frame = encryptor.encrypt(b"y").unwrap();
        assert_eq!(parse(&frame).unwrap().nonce, 0);
        assert_eq!(encryptor.generation, 256);
    }
}
