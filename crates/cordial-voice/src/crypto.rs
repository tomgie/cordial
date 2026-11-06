//! Transport encryption of RTP packets in the two rtpsize AEAD modes.
//!
//! Packet layout: `[unencrypted header] [ciphertext] [16-byte tag] [4-byte nonce]`.
//!
//! - The unencrypted header is the 12-byte RTP header, the CSRCs, and the extension
//!   preamble. It is the additional data of the AEAD.
//! - The nonce is a 32-bit counter. Its big-endian bytes are the first 4 bytes of a nonce
//!   that is otherwise zero: 12 bytes for AES-256-GCM, 24 bytes for XChaCha20-Poly1305.

use std::fmt;

use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{AeadInPlace, KeyInit};
use chacha20poly1305::XChaCha20Poly1305;
use zeroize::Zeroize;

use crate::packet::{PacketError, RtpHeader, RtpPacket};
use crate::protocol::EncryptionMode;

/// The size of the key of the transport encryption.
pub const KEY_LEN: usize = 32;
/// The size of the authentication tag.
pub const TAG_LEN: usize = 16;
/// The size of the nonce at the end of a packet.
pub const NONCE_SUFFIX_LEN: usize = 4;

/// The 32-byte key of the transport encryption. It is set to zero on drop.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretKey([u8; KEY_LEN]);

impl SecretKey {
    /// Makes a key from 32 bytes.
    #[must_use]
    pub const fn new(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Makes a key from a slice. Gives `None` when the slice does not have 32 bytes.
    #[must_use]
    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        bytes.try_into().ok().map(Self)
    }

    /// Gives the bytes of the key.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl Drop for SecretKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretKey(..)")
    }
}

/// An error of the transport encryption.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CryptoError {
    /// The packet is not a valid RTP packet.
    #[error(transparent)]
    Packet(#[from] PacketError),
    /// The packet is too short for a tag and a nonce.
    #[error("the packet is too short for a tag and a nonce")]
    TooShort,
    /// The tag is not correct: the packet was changed or the key is wrong.
    #[error("the packet cannot be decrypted")]
    Decrypt,
    /// The plaintext cannot be encrypted.
    #[error("the packet cannot be encrypted")]
    Encrypt,
    /// The padding or the extension is longer than the payload.
    #[error("the padding or the extension of the packet is not valid")]
    BadPayload,
}

/// A decrypted RTP packet.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct DecryptedPacket {
    /// The RTP header.
    pub header: RtpHeader,
    /// The payload without the extension body and without padding: an Opus frame, maybe
    /// with DAVE encryption.
    pub payload: Vec<u8>,
}

#[derive(Clone)]
enum Cipher {
    Aes(Box<Aes256Gcm>),
    XChaCha(Box<XChaCha20Poly1305>),
}

/// The transport cipher of a session.
#[derive(Clone)]
pub struct TransportCipher {
    mode: EncryptionMode,
    cipher: Cipher,
    nonce: u32,
}

impl fmt::Debug for TransportCipher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransportCipher")
            .field("mode", &self.mode)
            .field("nonce", &self.nonce)
            .finish_non_exhaustive()
    }
}

impl TransportCipher {
    /// Makes a cipher for a mode and a key. The nonce counter starts at 0.
    #[must_use]
    pub fn new(mode: EncryptionMode, key: &SecretKey) -> Self {
        let key = key.as_bytes();
        let cipher = match mode {
            EncryptionMode::Aes256GcmRtpSize => Cipher::Aes(Box::new(Aes256Gcm::new(key.into()))),
            EncryptionMode::XChaCha20Poly1305RtpSize => {
                Cipher::XChaCha(Box::new(XChaCha20Poly1305::new(key.into())))
            }
        };
        Self {
            mode,
            cipher,
            nonce: 0,
        }
    }

    /// Gives the mode.
    #[must_use]
    pub fn mode(&self) -> EncryptionMode {
        self.mode
    }

    /// Gives the next nonce counter value.
    #[must_use]
    pub fn next_nonce(&self) -> u32 {
        self.nonce
    }

    /// Encrypts a payload with the next nonce and increments the nonce counter.
    ///
    /// `header` is the unencrypted header (normally the 12 bytes of [`RtpHeader::encode`]).
    ///
    /// # Errors
    ///
    /// Gives an error when the AEAD fails.
    pub fn encrypt(&mut self, header: &[u8], payload: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let packet = self.encrypt_with_nonce(header, payload, self.nonce)?;
        self.nonce = self.nonce.wrapping_add(1);
        Ok(packet)
    }

    /// Encrypts a payload with a given nonce counter value.
    ///
    /// # Errors
    ///
    /// Gives an error when the AEAD fails.
    pub fn encrypt_with_nonce(
        &self,
        header: &[u8],
        payload: &[u8],
        nonce: u32,
    ) -> Result<Vec<u8>, CryptoError> {
        let mut packet =
            Vec::with_capacity(header.len() + payload.len() + TAG_LEN + NONCE_SUFFIX_LEN);
        packet.extend_from_slice(header);
        packet.extend_from_slice(payload);

        let (aad, body) = packet.split_at_mut(header.len());
        let tag = self.seal(nonce, aad, body)?;
        packet.extend_from_slice(&tag);
        packet.extend_from_slice(&nonce.to_be_bytes());
        Ok(packet)
    }

    /// Decrypts an RTP packet. Removes the extension body and the padding.
    ///
    /// # Errors
    ///
    /// Gives an error when the packet is not valid or the tag is not correct.
    pub fn decrypt(&self, packet: &[u8]) -> Result<DecryptedPacket, CryptoError> {
        let parsed = RtpPacket::parse(packet)?;
        let body = parsed.body;
        if body.len() < TAG_LEN + NONCE_SUFFIX_LEN {
            return Err(CryptoError::TooShort);
        }
        let (sealed, nonce) = body.split_at(body.len() - NONCE_SUFFIX_LEN);
        let nonce = u32::from_be_bytes([nonce[0], nonce[1], nonce[2], nonce[3]]);
        let (ciphertext, tag) = sealed.split_at(sealed.len() - TAG_LEN);

        let mut plaintext = ciphertext.to_vec();
        self.open(nonce, parsed.header_bytes, &mut plaintext, tag)?;

        let start = parsed
            .extension_words
            .map_or(0, |words| 4 * usize::from(words));
        let mut end = plaintext.len();
        if parsed.header.padding {
            let padding = usize::from(plaintext.last().copied().unwrap_or(0));
            end = end.checked_sub(padding).ok_or(CryptoError::BadPayload)?;
        }
        if start > end {
            return Err(CryptoError::BadPayload);
        }
        plaintext.truncate(end);
        plaintext.drain(..start);
        Ok(DecryptedPacket {
            header: parsed.header,
            payload: plaintext,
        })
    }

    fn seal(&self, nonce: u32, aad: &[u8], body: &mut [u8]) -> Result<[u8; TAG_LEN], CryptoError> {
        let tag = match &self.cipher {
            Cipher::Aes(cipher) => {
                let nonce = aes_nonce(nonce);
                cipher.encrypt_in_place_detached((&nonce).into(), aad, body)
            }
            Cipher::XChaCha(cipher) => {
                let nonce = xchacha_nonce(nonce);
                cipher.encrypt_in_place_detached((&nonce).into(), aad, body)
            }
        }
        .map_err(|_| CryptoError::Encrypt)?;
        Ok(tag.into())
    }

    fn open(&self, nonce: u32, aad: &[u8], body: &mut [u8], tag: &[u8]) -> Result<(), CryptoError> {
        match &self.cipher {
            Cipher::Aes(cipher) => {
                let nonce = aes_nonce(nonce);
                cipher.decrypt_in_place_detached((&nonce).into(), aad, body, tag.into())
            }
            Cipher::XChaCha(cipher) => {
                let nonce = xchacha_nonce(nonce);
                cipher.decrypt_in_place_detached((&nonce).into(), aad, body, tag.into())
            }
        }
        .map_err(|_| CryptoError::Decrypt)
    }
}

fn aes_nonce(counter: u32) -> [u8; 12] {
    let mut nonce = [0_u8; 12];
    nonce[..4].copy_from_slice(&counter.to_be_bytes());
    nonce
}

fn xchacha_nonce(counter: u32) -> [u8; 24] {
    let mut nonce = [0_u8; 24];
    nonce[..4].copy_from_slice(&counter.to_be_bytes());
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::aead::{Aead, Payload};

    fn key() -> SecretKey {
        let mut bytes = [0_u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::try_from(index).expect("small");
        }
        SecretKey::new(bytes)
    }

    #[test]
    fn round_trip_both_modes() {
        for mode in [
            EncryptionMode::Aes256GcmRtpSize,
            EncryptionMode::XChaCha20Poly1305RtpSize,
        ] {
            let mut cipher = TransportCipher::new(mode, &key());
            let header = RtpHeader::new(5, 960, 42).encode();
            let payload = [0xFC, 0x01, 0x02, 0x03];
            let first = cipher.encrypt(&header, &payload).expect("encrypt");
            let second = cipher.encrypt(&header, &payload).expect("encrypt");
            assert_eq!(first.len(), 12 + 4 + 16 + 4);
            assert_eq!(&first[first.len() - 4..], &[0, 0, 0, 0]);
            assert_eq!(&second[second.len() - 4..], &[0, 0, 0, 1]);
            assert_ne!(first[12..28], second[12..28]);

            let decrypted = cipher.decrypt(&first).expect("decrypt");
            assert_eq!(decrypted.payload, payload);
            assert_eq!(decrypted.header.sequence, 5);
            assert_eq!(decrypted.header.ssrc, 42);

            let mut changed = first.clone();
            changed[3] ^= 1;
            assert_eq!(cipher.decrypt(&changed), Err(CryptoError::Decrypt));
            let other = TransportCipher::new(mode, &SecretKey::new([9; 32]));
            assert_eq!(other.decrypt(&first), Err(CryptoError::Decrypt));
        }
    }

    /// Builds the packet by hand: header bytes, then the AEAD output of the cipher crate with
    /// a nonce and AAD laid out by hand, then the 4 nonce bytes.
    #[test]
    fn fixed_xchacha_packet() {
        let header: [u8; 12] = [
            0x80, 0x78, 0x00, 0x01, 0x00, 0x00, 0x03, 0xC0, 0x00, 0x00, 0x00, 0x2A,
        ];
        let payload = [0xF8, 0xFF, 0xFE];
        let mut nonce = [0_u8; 24];
        nonce[3] = 0x07;

        let reference = XChaCha20Poly1305::new((&[0x11_u8; 32]).into());
        let sealed = reference
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: &payload,
                    aad: &header,
                },
            )
            .expect("encrypt");
        let mut expected = header.to_vec();
        expected.extend_from_slice(&sealed);
        expected.extend_from_slice(&[0x00, 0x00, 0x00, 0x07]);

        let cipher = TransportCipher::new(
            EncryptionMode::XChaCha20Poly1305RtpSize,
            &SecretKey::new([0x11; 32]),
        );
        let packet = cipher
            .encrypt_with_nonce(&RtpHeader::new(1, 960, 42).encode(), &payload, 7)
            .expect("encrypt");
        assert_eq!(packet, expected);
        assert_eq!(packet.len(), 12 + 3 + 16 + 4);
        assert_eq!(cipher.decrypt(&packet).expect("decrypt").payload, payload);
    }

    /// A known answer for AES-256-GCM, computed with Node.js `crypto` (OpenSSL):
    /// key = 00..1f, nonce = 00 00 00 01 + 8 zero bytes, AAD = the 12-byte header below,
    /// plaintext = FC 01 02 03.
    #[test]
    fn known_aes_packet() {
        let header = RtpHeader::new(5, 960, 42).encode();
        let cipher = TransportCipher::new(EncryptionMode::Aes256GcmRtpSize, &key());
        let packet = cipher
            .encrypt_with_nonce(&header, &[0xFC, 0x01, 0x02, 0x03], 1)
            .expect("encrypt");
        let expected = hex(KNOWN_AES_PACKET);
        assert_eq!(packet, expected);
    }

    const KNOWN_AES_PACKET: &str =
        "80780005000003c00000002ab837cfa94208f258d83438a13f69fc2905d5f16900000001";

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).expect("hex"))
            .collect()
    }

    #[test]
    fn extension_and_padding_are_removed() {
        let cipher = TransportCipher::new(EncryptionMode::XChaCha20Poly1305RtpSize, &key());
        // Padding and extension bits; one CSRC; the extension body has 1 word.
        let mut header = vec![0xB1, 0x78, 0, 9, 0, 0, 0, 0, 0, 0, 0, 5];
        header.extend_from_slice(&[0, 0, 0, 77]);
        header.extend_from_slice(&[0xBE, 0xDE, 0x00, 0x01]);
        let mut plaintext = vec![0x10, 0xAA, 0xBB, 0xCC];
        plaintext.extend_from_slice(&[0xFC, 0xFF, 0xFE]);
        plaintext.extend_from_slice(&[0, 0, 3]);

        let packet = cipher
            .encrypt_with_nonce(&header, &plaintext, 3)
            .expect("encrypt");
        let decrypted = cipher.decrypt(&packet).expect("decrypt");
        assert_eq!(decrypted.payload, vec![0xFC, 0xFF, 0xFE]);
        assert_eq!(decrypted.header.csrc_count, 1);

        let mut bad = plaintext.clone();
        *bad.last_mut().expect("not empty") = 200;
        let packet = cipher
            .encrypt_with_nonce(&header, &bad, 4)
            .expect("encrypt");
        assert_eq!(cipher.decrypt(&packet), Err(CryptoError::BadPayload));
        assert_eq!(cipher.decrypt(&header), Err(CryptoError::TooShort));
    }
}
