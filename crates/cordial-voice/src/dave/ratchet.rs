//! The per-sender key ratchet.
//!
//! The ratchet is the MLS hash ratchet of RFC 9420, section 9.1, as libdave uses it through
//! mlspp `HashRatchet` (`mls_key_ratchet.cpp`; mlspp `key_schedule.cpp`, `crypto.cpp`):
//!
//! - `secret[0]` is the 16-byte base secret from the MLS exporter.
//! - `key[n] = ExpandWithLabel(secret[n], "key", n, 16)`.
//! - `secret[n + 1] = ExpandWithLabel(secret[n], "secret", n, 32)`.
//! - The context `n` is a big-endian `uint32` (mlspp `derive_tree_secret`, `tls::marshal`).
//! - `ExpandWithLabel` is HKDF-Expand with SHA-256 and the info
//!   `struct { uint16 length; opaque label<V> = "MLS 1.0 " + label; opaque context<V>; }`.
//!
//! mlspp also derives a nonce for each generation. DAVE does not use it, so this module skips it.
//! This does not change the keys.

use std::collections::BTreeMap;

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use super::cryptor::KEY_SIZE;

/// The output size of SHA-256, the MLS secret size of ciphersuite 2.
const SECRET_LENGTH: u16 = 32;

/// The AES-128 key size as an `ExpandWithLabel` length.
const KEY_LENGTH: u16 = 16;

/// Computes HKDF-Expand (RFC 5869) with HMAC-SHA-256.
///
/// The `hkdf` crate refuses a pseudorandom key shorter than the hash output. The DAVE base
/// secret has 16 bytes, so this function uses HMAC directly. HMAC accepts a key of any size.
fn hkdf_expand(secret: &[u8], info: &[u8], length: usize) -> Vec<u8> {
    let mut output = Vec::with_capacity(length);
    let mut previous: Vec<u8> = Vec::new();
    let mut counter: u8 = 1;
    while output.len() < length {
        let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret) else {
            // HMAC accepts all key sizes, so this does not occur.
            return output;
        };
        mac.update(&previous);
        mac.update(info);
        mac.update(&[counter]);
        previous = mac.finalize().into_bytes().to_vec();

        let needed = (length - output.len()).min(previous.len());
        output.extend_from_slice(&previous[..needed]);
        counter = counter.wrapping_add(1);
    }
    output
}

/// Appends an MLS variable-length vector (RFC 9420, section 2.1.2) to `out`.
fn write_vector(bytes: &[u8], out: &mut Vec<u8>) {
    // Labels and contexts are short. A length of 2^30 or more cannot occur.
    let length = u32::try_from(bytes.len()).unwrap_or(u32::MAX) & 0x3FFF_FFFF;
    if length < 0x40 {
        out.extend_from_slice(&length.to_be_bytes()[3..]);
    } else if length < 0x4000 {
        out.extend_from_slice(&(0x4000 | length).to_be_bytes()[2..]);
    } else {
        out.extend_from_slice(&(0x8000_0000 | length).to_be_bytes());
    }
    out.extend_from_slice(bytes);
}

/// Computes `ExpandWithLabel(secret, label, context, length)` of RFC 9420, section 8.
pub(crate) fn expand_with_label(
    secret: &[u8],
    label: &str,
    context: &[u8],
    length: u16,
) -> Vec<u8> {
    let full_label = format!("MLS 1.0 {label}");
    let mut info = Vec::with_capacity(2 + 1 + full_label.len() + 1 + context.len());
    info.extend_from_slice(&length.to_be_bytes());
    write_vector(full_label.as_bytes(), &mut info);
    write_vector(context, &mut info);
    hkdf_expand(secret, &info, usize::from(length))
}

/// A hash ratchet that gives the AES-128 key of each generation.
pub(crate) struct KeyRatchet {
    base_secret: Vec<u8>,
    next_secret: Vec<u8>,
    next_generation: u64,
    keys: BTreeMap<u32, [u8; KEY_SIZE]>,
}

impl std::fmt::Debug for KeyRatchet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyRatchet")
            .field("next_generation", &self.next_generation)
            .finish_non_exhaustive()
    }
}

impl KeyRatchet {
    /// Makes a ratchet from the base secret.
    pub(crate) fn new(base_secret: Vec<u8>) -> Self {
        Self {
            next_secret: base_secret.clone(),
            base_secret,
            next_generation: 0,
            keys: BTreeMap::new(),
        }
    }

    /// Gives the value that identifies the key domain (one sender in one epoch). libdave uses
    /// the base secret (`MlsKeyRatchet::GetDomainIdentity`).
    pub(crate) fn domain(&self) -> &[u8] {
        &self.base_secret
    }

    /// Gives the key of a generation. Gives `None` when the key of that generation is erased.
    pub(crate) fn get(&mut self, generation: u32) -> Option<[u8; KEY_SIZE]> {
        if let Some(key) = self.keys.get(&generation) {
            return Some(*key);
        }
        if u64::from(generation) < self.next_generation {
            return None;
        }

        while self.next_generation <= u64::from(generation) {
            self.advance();
        }
        self.keys.get(&generation).copied()
    }

    /// Erases the key of a generation.
    pub(crate) fn erase(&mut self, generation: u32) {
        self.keys.remove(&generation);
    }

    fn advance(&mut self) {
        // `get` only advances up to a `u32` generation, so the value fits.
        let generation = u32::try_from(self.next_generation).unwrap_or(u32::MAX);
        let context = generation.to_be_bytes();

        let key_bytes = expand_with_label(&self.next_secret, "key", &context, KEY_LENGTH);
        let mut key = [0; KEY_SIZE];
        key.copy_from_slice(&key_bytes);
        self.next_secret = expand_with_label(&self.next_secret, "secret", &context, SECRET_LENGTH);

        self.keys.insert(generation, key);
        self.next_generation += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn hkdf_expand_matches_rfc_5869_case_1() {
        let prk = hex("077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5");
        let info = hex("f0f1f2f3f4f5f6f7f8f9");
        let okm = hkdf_expand(&prk, &info, 42);
        assert_eq!(
            okm,
            hex(
                "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
            )
        );
    }

    #[test]
    fn mls_vector_lengths() {
        let mut out = Vec::new();
        write_vector(&[0; 0x3F], &mut out);
        assert_eq!(out[0], 0x3F);

        let mut out = Vec::new();
        write_vector(&[0; 0x40], &mut out);
        assert_eq!(&out[..2], &[0x40, 0x40]);

        let mut out = Vec::new();
        write_vector(&[0; 0x4000], &mut out);
        assert_eq!(&out[..4], &[0x80, 0x00, 0x40, 0x00]);
    }

    #[test]
    fn expand_with_label_info_layout() {
        // The info of ExpandWithLabel(secret, "key", 0x00000001, 16).
        let mut info = Vec::new();
        info.extend_from_slice(&16u16.to_be_bytes());
        info.push(11);
        info.extend_from_slice(b"MLS 1.0 key");
        info.extend_from_slice(&[4, 0, 0, 0, 1]);
        let secret = [3; 16];
        assert_eq!(
            expand_with_label(&secret, "key", &[0, 0, 0, 1], 16),
            hkdf_expand(&secret, &info, 16)
        );
    }

    /// Known answers from a separate implementation of RFC 9420 `ExpandWithLabel` and the
    /// mlspp `HashRatchet` chain (Python standard library `hmac` and `hashlib`).
    #[test]
    fn ratchet_known_answers() {
        let mut ratchet = KeyRatchet::new(vec![0xA5; 16]);
        let expected = [
            (0, "0225f172a1b7d618684f13f78fbf8c5e"),
            (1, "18375aa7ebfd4fc6cd3e8da45d2fd6f7"),
            (2, "faf9476e61f6164d5d530a65ee461f54"),
            (255, "d82347e4c3cc4986b1ca3c9b20cb5fcc"),
            (256, "e94c3dc442f4999e9acaf3693df66e27"),
            (300, "a5f26c20f286c8b899101841ac8f652e"),
        ];
        for (generation, key) in expected {
            assert_eq!(
                ratchet.get(generation).unwrap().to_vec(),
                hex(key),
                "generation {generation}"
            );
        }
    }

    #[test]
    fn ratchet_chain() {
        let base = vec![0xA5; 16];
        let mut ratchet = KeyRatchet::new(base.clone());

        let key0 = expand_with_label(&base, "key", &0u32.to_be_bytes(), 16);
        let secret1 = expand_with_label(&base, "secret", &0u32.to_be_bytes(), 32);
        let key1 = expand_with_label(&secret1, "key", &1u32.to_be_bytes(), 16);
        let secret2 = expand_with_label(&secret1, "secret", &1u32.to_be_bytes(), 32);
        let key2 = expand_with_label(&secret2, "key", &2u32.to_be_bytes(), 16);

        assert_eq!(ratchet.get(2).unwrap().to_vec(), key2);
        assert_eq!(ratchet.get(0).unwrap().to_vec(), key0);
        assert_eq!(ratchet.get(1).unwrap().to_vec(), key1);

        ratchet.erase(0);
        assert!(ratchet.get(0).is_none());
        assert_eq!(ratchet.get(1).unwrap().to_vec(), key1);
        assert_eq!(ratchet.domain(), &base[..]);
    }
}
