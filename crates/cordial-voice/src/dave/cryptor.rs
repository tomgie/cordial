//! AES-128-GCM with a tag truncated to 8 bytes.
//!
//! The RustCrypto `aes-gcm` crate supports tags of 12 to 16 bytes only. A truncated GCM tag is
//! the prefix of the full tag (NIST SP 800-38D, section 5.2.1.2). Thus this module makes the full
//! tag and keeps its first 8 bytes. To decrypt, it uses the counter mode keystream of GCM to get
//! the plaintext, encrypts the plaintext again to get the full tag, and compares the prefix in
//! constant time. It gives the plaintext only when the tag is correct.

use aes_gcm::aead::{AeadInPlace, KeyInit};
use aes_gcm::{Aes128Gcm, Nonce};
use subtle::ConstantTimeEq;

use super::frame::TAG_SIZE;

/// The size of an AES-128 key.
pub(crate) const KEY_SIZE: usize = 16;

/// Makes the 12-byte GCM nonce from the 32-bit truncated nonce.
///
/// libdave copies the `uint32_t` into bytes 8 to 11 with `memcpy` (`encryptor.cpp`,
/// `decryptor.cpp`, offset `kAesGcm128TruncatedSyncNonceOffset` in `common.h`). On the
/// little-endian hosts that Discord supports, this is the little-endian byte order. Bytes 0 to 7
/// are zero.
pub(crate) fn full_nonce(truncated_nonce: u32) -> [u8; 12] {
    let mut nonce = [0; 12];
    nonce[8..].copy_from_slice(&truncated_nonce.to_le_bytes());
    nonce
}

/// An AES-128-GCM cipher for one key.
pub(crate) struct Cryptor {
    cipher: Aes128Gcm,
}

impl std::fmt::Debug for Cryptor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cryptor").finish_non_exhaustive()
    }
}

impl Cryptor {
    /// Makes a cipher for a key.
    pub(crate) fn new(key: &[u8; KEY_SIZE]) -> Self {
        Self {
            cipher: Aes128Gcm::new(key.into()),
        }
    }

    /// Encrypts `plaintext`. Gives the ciphertext and the truncated tag.
    pub(crate) fn encrypt(
        &self,
        truncated_nonce: u32,
        authenticated_data: &[u8],
        plaintext: &[u8],
    ) -> Option<(Vec<u8>, [u8; TAG_SIZE])> {
        let nonce = full_nonce(truncated_nonce);
        let mut buffer = plaintext.to_vec();
        let full_tag = self
            .cipher
            .encrypt_in_place_detached(Nonce::from_slice(&nonce), authenticated_data, &mut buffer)
            .ok()?;

        let mut tag = [0; TAG_SIZE];
        tag.copy_from_slice(&full_tag[..TAG_SIZE]);
        Some((buffer, tag))
    }

    /// Decrypts `ciphertext`. Gives `None` when the tag is not correct.
    pub(crate) fn decrypt(
        &self,
        truncated_nonce: u32,
        authenticated_data: &[u8],
        ciphertext: &[u8],
        tag: &[u8; TAG_SIZE],
    ) -> Option<Vec<u8>> {
        let nonce = full_nonce(truncated_nonce);
        let nonce = Nonce::from_slice(&nonce);

        // GCM encryption is the counter mode keystream XOR the input. Thus the encryption of
        // the ciphertext gives the candidate plaintext. Its tag has no use.
        let mut plaintext = ciphertext.to_vec();
        self.cipher
            .encrypt_in_place_detached(nonce, &[], &mut plaintext)
            .ok()?;

        // The encryption of the candidate plaintext gives the ciphertext again and its full tag.
        let mut check = plaintext.clone();
        let full_tag = self
            .cipher
            .encrypt_in_place_detached(nonce, authenticated_data, &mut check)
            .ok()?;

        let tag_matches: bool = full_tag[..TAG_SIZE].ct_eq(tag).into();
        tag_matches.then_some(plaintext)
    }
}

#[cfg(test)]
mod tests {
    use aes_gcm::aead::Aead;
    use aes_gcm::aead::Payload;

    use super::*;

    #[test]
    fn nonce_layout() {
        assert_eq!(
            full_nonce(0x0403_0201),
            [0, 0, 0, 0, 0, 0, 0, 0, 0x01, 0x02, 0x03, 0x04]
        );
    }

    #[test]
    fn round_trip_with_and_without_authenticated_data() {
        let cryptor = Cryptor::new(&[7; KEY_SIZE]);
        for aad in [&[][..], &[1, 2, 3][..]] {
            let (ciphertext, tag) = cryptor.encrypt(5, aad, b"hello opus").unwrap();
            assert_eq!(ciphertext.len(), 10);
            assert_eq!(
                cryptor.decrypt(5, aad, &ciphertext, &tag).unwrap(),
                b"hello opus"
            );
            assert!(cryptor.decrypt(6, aad, &ciphertext, &tag).is_none());
            assert!(cryptor.decrypt(5, &[9], &ciphertext, &tag).is_none());

            let mut bad_tag = tag;
            bad_tag[0] ^= 1;
            assert!(cryptor.decrypt(5, aad, &ciphertext, &bad_tag).is_none());

            let mut bad_ciphertext = ciphertext.clone();
            bad_ciphertext[3] ^= 1;
            assert!(cryptor.decrypt(5, aad, &bad_ciphertext, &tag).is_none());
        }
    }

    #[test]
    fn truncated_tag_is_prefix_of_standard_tag() {
        let key = [0x42; KEY_SIZE];
        let cryptor = Cryptor::new(&key);
        let (ciphertext, tag) = cryptor.encrypt(1, b"aad", b"plaintext").unwrap();

        // A standard AES-128-GCM gives the ciphertext and the full 16-byte tag.
        let standard = Aes128Gcm::new((&key).into())
            .encrypt(
                Nonce::from_slice(&full_nonce(1)),
                Payload {
                    msg: b"plaintext",
                    aad: b"aad",
                },
            )
            .unwrap();
        assert_eq!(&standard[..9], &ciphertext[..]);
        assert_eq!(&standard[9..9 + TAG_SIZE], &tag);
    }
}
