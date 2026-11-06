//! The OpenMLS provider of DAVE: cryptography, randomness, and storage.
//!
//! DAVE protocol version 1 uses one MLS ciphersuite only:
//! `MLS_128_DHKEMP256_AES128GCM_SHA256_P256` (ciphersuite 2). Thus this provider supports
//! only these algorithms. Other algorithms give an "unsupported" error.
//!
//! | Function | Algorithm | Crate |
//! |---|---|---|
//! | Hash, HMAC, HKDF | SHA-256 (RFC 5869) | `sha2`, `hmac` |
//! | AEAD | AES-128-GCM | `aes-gcm` |
//! | Signature | ECDSA P-256 with SHA-256, DER encoding, SEC1 uncompressed keys | `p256` |
//! | HPKE | DHKEM(P-256, HKDF-SHA256), HKDF-SHA256, AES-128-GCM, base mode (RFC 9180) | `hpke` |
//! | Randomness | The random generator of the operating system | `getrandom` |
//! | Storage | Memory | `openmls_memory_storage` |
//!
//! The key formats are the same as those of mlspp (libdave): a private key is the 32-byte
//! scalar, and a public key is the 65-byte uncompressed SEC1 point.

use std::fmt;

use aes_gcm::aead::{Aead, KeyInit as _, Payload};
use aes_gcm::{Aes128Gcm, Nonce};
use hmac::{Hmac, KeyInit, Mac};
use hpke::aead::AesGcm128;
use hpke::kdf::HkdfSha256;
use hpke::kem::DhP256HkdfSha256;
use hpke::{Deserializable, Kem as _, OpModeR, OpModeS, Serializable};
use openmls::prelude::tls_codec::SecretVLBytes;
use openmls_memory_storage::MemoryStorage;
use openmls_traits::OpenMlsProvider;
use openmls_traits::crypto::OpenMlsCrypto;
use openmls_traits::random::OpenMlsRand;
use openmls_traits::signatures::{Signer, SignerError};
use openmls_traits::types::{
    AeadType, Ciphersuite, CryptoError, ExporterSecret, HashType, HpkeAeadType, HpkeCiphertext,
    HpkeConfig, HpkeKdfType, HpkeKemType, HpkeKeyPair, KemOutput, SignatureScheme,
};
use p256::ecdsa::signature::{Signer as _, Verifier as _};
use p256::ecdsa::{Signature, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

/// The only supported MLS ciphersuite.
pub(crate) const CIPHERSUITE: Ciphersuite = Ciphersuite::MLS_128_DHKEMP256_AES128GCM_SHA256_P256;

/// The only supported signature scheme.
pub(crate) const SIGNATURE_SCHEME: SignatureScheme = SignatureScheme::ECDSA_SECP256R1_SHA256;

/// The size of a SHA-256 output.
const HASH_SIZE: usize = 32;

/// The size of a P-256 private key (a scalar).
const PRIVATE_KEY_SIZE: usize = 32;

/// The size of an AES-128-GCM key.
const AEAD_KEY_SIZE: usize = 16;

/// The size of an AES-128-GCM nonce.
const AEAD_NONCE_SIZE: usize = 12;

type Kem = DhP256HkdfSha256;

/// Computes HMAC-SHA-256.
fn hmac_sha256(key: &[u8], parts: &[&[u8]]) -> Result<[u8; HASH_SIZE], CryptoError> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key).map_err(|_| CryptoError::CryptoLibraryError)?;
    for part in parts {
        mac.update(part);
    }
    Ok(mac.finalize().into_bytes().into())
}

/// Computes HKDF-Expand (RFC 5869) with SHA-256.
///
/// The `hkdf` crate refuses a pseudorandom key shorter than the hash output. The DAVE base
/// secret has 16 bytes, so this function uses HMAC directly. HMAC accepts a key of any size.
pub(crate) fn hkdf_expand_sha256(
    prk: &[u8],
    info: &[u8],
    length: usize,
) -> Result<Vec<u8>, CryptoError> {
    if length > 255 * HASH_SIZE {
        return Err(CryptoError::HkdfOutputLengthInvalid);
    }

    let mut output = Vec::with_capacity(length);
    let mut previous: Vec<u8> = Vec::new();
    let mut counter: u8 = 1;
    while output.len() < length {
        let block = hmac_sha256(prk, &[&previous, info, &[counter]])?;
        let needed = (length - output.len()).min(HASH_SIZE);
        output.extend_from_slice(&block[..needed]);
        previous = block.to_vec();
        counter = counter.wrapping_add(1);
    }
    Ok(output)
}

/// Computes HKDF-Extract (RFC 5869) with SHA-256. An empty salt is the same as a salt of
/// zeros, because HMAC pads the key with zeros.
fn hkdf_extract_sha256(salt: &[u8], ikm: &[u8]) -> Result<[u8; HASH_SIZE], CryptoError> {
    hmac_sha256(salt, &[ikm])
}

/// Fills a buffer from the random generator of the operating system.
fn fill_random(buffer: &mut [u8]) -> Result<(), RandError> {
    getrandom::fill(buffer).map_err(|_| RandError)
}

fn check_hash(hash_type: HashType) -> Result<(), CryptoError> {
    match hash_type {
        HashType::Sha2_256 => Ok(()),
        _ => Err(CryptoError::UnsupportedHashAlgorithm),
    }
}

fn check_signature_scheme(scheme: SignatureScheme) -> Result<(), CryptoError> {
    if scheme == SIGNATURE_SCHEME {
        Ok(())
    } else {
        Err(CryptoError::UnsupportedSignatureScheme)
    }
}

fn check_hpke(config: &HpkeConfig) -> Result<(), CryptoError> {
    let HpkeConfig(kem, kdf, aead) = config;
    if *kem != HpkeKemType::DhKemP256 {
        return Err(CryptoError::UnsupportedCiphersuite);
    }
    if *kdf != HpkeKdfType::HkdfSha256 {
        return Err(CryptoError::UnsupportedKdf);
    }
    if *aead != HpkeAeadType::AesGcm128 {
        return Err(CryptoError::UnsupportedAeadAlgorithm);
    }
    Ok(())
}

fn aead_cipher(alg: AeadType, key: &[u8], nonce: &[u8]) -> Result<Aes128Gcm, CryptoError> {
    if alg != AeadType::Aes128Gcm {
        return Err(CryptoError::UnsupportedAeadAlgorithm);
    }
    if key.len() != AEAD_KEY_SIZE || nonce.len() != AEAD_NONCE_SIZE {
        return Err(CryptoError::InvalidLength);
    }
    Aes128Gcm::new_from_slice(key).map_err(|_| CryptoError::InvalidLength)
}

fn hpke_public_key(bytes: &[u8]) -> Result<<Kem as hpke::Kem>::PublicKey, CryptoError> {
    <Kem as hpke::Kem>::PublicKey::from_bytes(bytes).map_err(|_| CryptoError::InvalidPublicKey)
}

fn hpke_private_key(bytes: &[u8]) -> Result<<Kem as hpke::Kem>::PrivateKey, CryptoError> {
    <Kem as hpke::Kem>::PrivateKey::from_bytes(bytes).map_err(|_| CryptoError::InvalidLength)
}

fn hpke_encapped_key(bytes: &[u8]) -> Result<<Kem as hpke::Kem>::EncappedKey, CryptoError> {
    <Kem as hpke::Kem>::EncappedKey::from_bytes(bytes).map_err(|_| CryptoError::InvalidLength)
}

/// Makes a new ECDSA P-256 signing key from the random generator of the operating system.
fn random_signing_key() -> Result<SigningKey, CryptoError> {
    // A random 32-byte value is a valid scalar with a probability of almost 1. The loop
    // rejects the rare values that are zero or not below the group order.
    for _ in 0..16 {
        let mut bytes = [0; PRIVATE_KEY_SIZE];
        fill_random(&mut bytes).map_err(|_| CryptoError::InsufficientRandomness)?;
        if let Ok(key) = SigningKey::from_slice(&bytes) {
            return Ok(key);
        }
    }
    Err(CryptoError::InsufficientRandomness)
}

fn public_key_bytes(key: &SigningKey) -> Vec<u8> {
    key.verifying_key().to_sec1_point(false).as_bytes().to_vec()
}

fn sign_with(key: &SigningKey, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let signature: Signature = key.try_sign(data).map_err(|_| CryptoError::SigningError)?;
    Ok(signature.to_der().as_bytes().to_vec())
}

/// An error of the random generator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RandError;

impl fmt::Display for RandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the random generator of the operating system failed")
    }
}

impl std::error::Error for RandError {}

/// The cryptography and randomness of the provider. It has no state.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DaveCrypto;

impl OpenMlsRand for DaveCrypto {
    type Error = RandError;

    fn random_array<const N: usize>(&self) -> Result<[u8; N], Self::Error> {
        let mut output = [0; N];
        fill_random(&mut output)?;
        Ok(output)
    }

    fn random_vec(&self, len: usize) -> Result<Vec<u8>, Self::Error> {
        let mut output = vec![0; len];
        fill_random(&mut output)?;
        Ok(output)
    }
}

impl OpenMlsCrypto for DaveCrypto {
    fn supports(&self, ciphersuite: Ciphersuite) -> Result<(), CryptoError> {
        if ciphersuite == CIPHERSUITE {
            Ok(())
        } else {
            Err(CryptoError::UnsupportedCiphersuite)
        }
    }

    fn supported_ciphersuites(&self) -> Vec<Ciphersuite> {
        vec![CIPHERSUITE]
    }

    fn hkdf_extract(
        &self,
        hash_type: HashType,
        salt: &[u8],
        ikm: &[u8],
    ) -> Result<SecretVLBytes, CryptoError> {
        check_hash(hash_type)?;
        Ok(hkdf_extract_sha256(salt, ikm)?.to_vec().into())
    }

    fn hmac(
        &self,
        hash_type: HashType,
        key: &[u8],
        message: &[u8],
    ) -> Result<SecretVLBytes, CryptoError> {
        check_hash(hash_type)?;
        Ok(hmac_sha256(key, &[message])?.to_vec().into())
    }

    fn hkdf_expand(
        &self,
        hash_type: HashType,
        prk: &[u8],
        info: &[u8],
        okm_len: usize,
    ) -> Result<SecretVLBytes, CryptoError> {
        check_hash(hash_type)?;
        Ok(hkdf_expand_sha256(prk, info, okm_len)?.into())
    }

    fn hash(&self, hash_type: HashType, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        check_hash(hash_type)?;
        Ok(Sha256::digest(data).to_vec())
    }

    fn aead_encrypt(
        &self,
        alg: AeadType,
        key: &[u8],
        data: &[u8],
        nonce: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let cipher = aead_cipher(alg, key, nonce)?;
        cipher
            .encrypt(Nonce::from_slice(nonce), Payload { msg: data, aad })
            .map_err(|_| CryptoError::CryptoLibraryError)
    }

    fn aead_decrypt(
        &self,
        alg: AeadType,
        key: &[u8],
        ct_tag: &[u8],
        nonce: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let cipher = aead_cipher(alg, key, nonce)?;
        cipher
            .decrypt(Nonce::from_slice(nonce), Payload { msg: ct_tag, aad })
            .map_err(|_| CryptoError::AeadDecryptionError)
    }

    fn signature_key_gen(&self, alg: SignatureScheme) -> Result<(Vec<u8>, Vec<u8>), CryptoError> {
        check_signature_scheme(alg)?;
        let key = random_signing_key()?;
        Ok((key.to_bytes().to_vec(), public_key_bytes(&key)))
    }

    fn verify_signature(
        &self,
        alg: SignatureScheme,
        data: &[u8],
        pk: &[u8],
        signature: &[u8],
    ) -> Result<(), CryptoError> {
        check_signature_scheme(alg)?;
        let key = VerifyingKey::from_sec1_bytes(pk).map_err(|_| CryptoError::InvalidPublicKey)?;
        let signature =
            Signature::from_der(signature).map_err(|_| CryptoError::SignatureDecodingError)?;
        key.verify(data, &signature)
            .map_err(|_| CryptoError::InvalidSignature)
    }

    fn sign(&self, alg: SignatureScheme, data: &[u8], key: &[u8]) -> Result<Vec<u8>, CryptoError> {
        check_signature_scheme(alg)?;
        let key = SigningKey::from_slice(key).map_err(|_| CryptoError::InvalidLength)?;
        sign_with(&key, data)
    }

    fn hpke_seal(
        &self,
        config: HpkeConfig,
        pk_r: &[u8],
        info: &[u8],
        aad: &[u8],
        ptxt: &[u8],
    ) -> Result<HpkeCiphertext, CryptoError> {
        check_hpke(&config)?;
        let public_key = hpke_public_key(pk_r)?;
        let (encapped_key, ciphertext) = hpke::single_shot_seal::<AesGcm128, HkdfSha256, Kem>(
            &OpModeS::Base,
            &public_key,
            info,
            ptxt,
            aad,
        )
        .map_err(|_| CryptoError::HpkeEncryptionError)?;
        Ok(HpkeCiphertext {
            kem_output: encapped_key.to_bytes().to_vec().into(),
            ciphertext: ciphertext.into(),
        })
    }

    fn hpke_open(
        &self,
        config: HpkeConfig,
        input: &HpkeCiphertext,
        sk_r: &[u8],
        info: &[u8],
        aad: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        check_hpke(&config)?;
        let private_key = hpke_private_key(sk_r)?;
        let encapped_key = hpke_encapped_key(input.kem_output.as_slice())?;
        hpke::single_shot_open::<AesGcm128, HkdfSha256, Kem>(
            &OpModeR::Base,
            &private_key,
            &encapped_key,
            info,
            input.ciphertext.as_slice(),
            aad,
        )
        .map_err(|_| CryptoError::HpkeDecryptionError)
    }

    fn hpke_setup_sender_and_export(
        &self,
        config: HpkeConfig,
        pk_r: &[u8],
        info: &[u8],
        exporter_context: &[u8],
        exporter_length: usize,
    ) -> Result<(KemOutput, ExporterSecret), CryptoError> {
        check_hpke(&config)?;
        let public_key = hpke_public_key(pk_r)?;
        let (encapped_key, context) =
            hpke::setup_sender::<AesGcm128, HkdfSha256, Kem>(&OpModeS::Base, &public_key, info)
                .map_err(|_| CryptoError::SenderSetupError)?;
        let mut secret = vec![0; exporter_length];
        context
            .export(exporter_context, &mut secret)
            .map_err(|_| CryptoError::ExporterError)?;
        Ok((encapped_key.to_bytes().to_vec(), secret.into()))
    }

    fn hpke_setup_receiver_and_export(
        &self,
        config: HpkeConfig,
        enc: &[u8],
        sk_r: &[u8],
        info: &[u8],
        exporter_context: &[u8],
        exporter_length: usize,
    ) -> Result<ExporterSecret, CryptoError> {
        check_hpke(&config)?;
        let private_key = hpke_private_key(sk_r)?;
        let encapped_key = hpke_encapped_key(enc)?;
        let context = hpke::setup_receiver::<AesGcm128, HkdfSha256, Kem>(
            &OpModeR::Base,
            &private_key,
            &encapped_key,
            info,
        )
        .map_err(|_| CryptoError::ReceiverSetupError)?;
        let mut secret = vec![0; exporter_length];
        context
            .export(exporter_context, &mut secret)
            .map_err(|_| CryptoError::ExporterError)?;
        Ok(secret.into())
    }

    fn derive_hpke_keypair(
        &self,
        config: HpkeConfig,
        ikm: &[u8],
    ) -> Result<HpkeKeyPair, CryptoError> {
        check_hpke(&config)?;
        // RFC 9180 requires at least as many input bytes as the private key has.
        if ikm.len() < PRIVATE_KEY_SIZE {
            return Err(CryptoError::InvalidLength);
        }
        let (private_key, public_key) = Kem::derive_keypair(ikm);
        Ok(HpkeKeyPair {
            private: private_key.to_bytes().to_vec().into(),
            public: public_key.to_bytes().to_vec(),
        })
    }
}

/// The OpenMLS provider of a DAVE group: [`DaveCrypto`] and a memory storage.
#[derive(Debug, Default)]
pub(crate) struct DaveProvider {
    crypto: DaveCrypto,
    storage: MemoryStorage,
}

impl OpenMlsProvider for DaveProvider {
    type CryptoProvider = DaveCrypto;
    type RandProvider = DaveCrypto;
    type StorageProvider = MemoryStorage;

    fn storage(&self) -> &Self::StorageProvider {
        &self.storage
    }

    fn crypto(&self) -> &Self::CryptoProvider {
        &self.crypto
    }

    fn rand(&self) -> &Self::RandProvider {
        &self.crypto
    }
}

/// An ECDSA P-256 signature key pair. It signs MLS messages for a basic credential.
pub(crate) struct SignatureKeyPair {
    private: SigningKey,
    public: Vec<u8>,
}

impl fmt::Debug for SignatureKeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignatureKeyPair")
            .field("public", &self.public)
            .finish_non_exhaustive()
    }
}

impl SignatureKeyPair {
    /// Makes a new key pair from the random generator of the operating system.
    pub(crate) fn generate() -> Result<Self, CryptoError> {
        let private = random_signing_key()?;
        let public = public_key_bytes(&private);
        Ok(Self { private, public })
    }

    /// Gives the public key as an uncompressed SEC1 point.
    pub(crate) fn public(&self) -> &[u8] {
        &self.public
    }
}

impl Signer for SignatureKeyPair {
    fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, SignerError> {
        sign_with(&self.private, payload).map_err(SignerError::CryptoError)
    }

    fn signature_scheme(&self) -> SignatureScheme {
        SIGNATURE_SCHEME
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

    fn hpke_config() -> HpkeConfig {
        HpkeConfig(
            HpkeKemType::DhKemP256,
            HpkeKdfType::HkdfSha256,
            HpkeAeadType::AesGcm128,
        )
    }

    /// RFC 5869, appendix A.1 (test case 1).
    #[test]
    fn hkdf_matches_rfc_5869_case_1() {
        let crypto = DaveCrypto;
        let ikm = hex("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b");
        let salt = hex("000102030405060708090a0b0c");
        let info = hex("f0f1f2f3f4f5f6f7f8f9");

        let prk = crypto
            .hkdf_extract(HashType::Sha2_256, &salt, &ikm)
            .unwrap();
        assert_eq!(
            prk.as_slice(),
            hex("077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5")
        );
        let okm = crypto
            .hkdf_expand(HashType::Sha2_256, prk.as_slice(), &info, 42)
            .unwrap();
        assert_eq!(
            okm.as_slice(),
            hex(
                "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
            )
        );
    }

    /// RFC 5869, appendix A.3 (test case 3: no salt, no info).
    #[test]
    fn hkdf_matches_rfc_5869_case_3() {
        let crypto = DaveCrypto;
        let ikm = hex("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b");
        let prk = crypto.hkdf_extract(HashType::Sha2_256, &[], &ikm).unwrap();
        assert_eq!(
            prk.as_slice(),
            hex("19ef24a32c717b167f33a91d6f648bdf96596776afdb6377ac434c1c293ccb04")
        );
        let okm = crypto
            .hkdf_expand(HashType::Sha2_256, prk.as_slice(), &[], 42)
            .unwrap();
        assert_eq!(
            okm.as_slice(),
            hex(
                "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8"
            )
        );
    }

    #[test]
    fn hash_and_aead() {
        let crypto = DaveCrypto;
        assert_eq!(
            crypto.hash(HashType::Sha2_256, b"abc").unwrap(),
            hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );

        let key = [7; 16];
        let nonce = [9; 12];
        let sealed = crypto
            .aead_encrypt(AeadType::Aes128Gcm, &key, b"data", &nonce, b"aad")
            .unwrap();
        assert_eq!(sealed.len(), 4 + 16);
        assert_eq!(
            crypto
                .aead_decrypt(AeadType::Aes128Gcm, &key, &sealed, &nonce, b"aad")
                .unwrap(),
            b"data"
        );
        assert_eq!(
            crypto.aead_decrypt(AeadType::Aes128Gcm, &key, &sealed, &nonce, b"bad"),
            Err(CryptoError::AeadDecryptionError)
        );
    }

    /// The ECDSA P-256 SHA-256 known answer of RFC 6979, appendix A.2.5, message "sample".
    /// ECDSA signing in `p256` is deterministic as RFC 6979 describes.
    #[test]
    fn ecdsa_matches_rfc_6979() {
        let crypto = DaveCrypto;
        let private = hex("c9afa9d845ba75166b5c215767b1d6934e50c3db36e89b127b8a622b120f6721");
        let public = {
            let mut bytes = vec![0x04];
            bytes.extend(hex(
                "60fed4ba255a9d31c961eb74c6356d68c049b8923b61fa6ce669622e60f29fb6",
            ));
            bytes.extend(hex(
                "7903fe1008b8bc99a41ae9e95628bc64f2f1b20c2d7e9f5177a3c294d4462299",
            ));
            bytes
        };
        let key = SigningKey::from_slice(&private).unwrap();
        assert_eq!(public_key_bytes(&key), public);

        let der = crypto.sign(SIGNATURE_SCHEME, b"sample", &private).unwrap();
        let signature = Signature::from_der(&der).unwrap();
        assert_eq!(
            signature.to_bytes().to_vec(),
            hex(concat!(
                "efd48b2aacb6a8fd1140dd9cd45e81d69d2c877b56aaf991c34d0ea84eaf3716",
                "f7cb1c942d657c41d436c7a1b6e29f65f3e900dbb9aff4064dc4ab2f843acda8"
            ))
        );
        crypto
            .verify_signature(SIGNATURE_SCHEME, b"sample", &public, &der)
            .unwrap();
        assert_eq!(
            crypto.verify_signature(SIGNATURE_SCHEME, b"other", &public, &der),
            Err(CryptoError::InvalidSignature)
        );
    }

    #[test]
    fn generated_signature_keys_sign_and_verify() {
        let crypto = DaveCrypto;
        let (private, public) = crypto.signature_key_gen(SIGNATURE_SCHEME).unwrap();
        assert_eq!(private.len(), 32);
        assert_eq!(public.len(), 65);
        assert_eq!(public[0], 0x04);
        let signature = crypto.sign(SIGNATURE_SCHEME, b"payload", &private).unwrap();
        crypto
            .verify_signature(SIGNATURE_SCHEME, b"payload", &public, &signature)
            .unwrap();

        let pair = SignatureKeyPair::generate().unwrap();
        let signature = Signer::sign(&pair, b"payload").unwrap();
        crypto
            .verify_signature(SIGNATURE_SCHEME, b"payload", pair.public(), &signature)
            .unwrap();
    }

    /// RFC 9180, appendix A.3.1: DHKEM(P-256, HKDF-SHA256), HKDF-SHA256, AES-128-GCM, base
    /// mode. The derived key pair of the recipient and the first sealed message.
    #[test]
    fn hpke_matches_rfc_9180_a_3_1() {
        let crypto = DaveCrypto;
        let ikm_recipient = hex("668b37171f1072f3cf12ea8a236a45df23fc13b82af3609ad1e354f6ef817550");
        let pair = crypto
            .derive_hpke_keypair(hpke_config(), &ikm_recipient)
            .unwrap();
        assert_eq!(
            pair.private.to_vec(),
            hex("f3ce7fdae57e1a310d87f1ebbde6f328be0a99cdbcadf4d6589cf29de4b8ffd2")
        );
        assert_eq!(
            pair.public,
            hex(concat!(
                "04fe8c19ce0905191ebc298a9245792531f26f0cece2460639e8bc39cb7f706a82",
                "6a779b4cf969b8a0e539c7f62fb3d30ad6aa8f80e30f1d128aafd68a2ce72ea0"
            ))
        );

        // The sender's ephemeral key is random in `hpke_seal`, so open the known ciphertext.
        let enc = hex(concat!(
            "04a92719c6195d5085104f469a8b9814d5838ff72b60501e2c4466e5e67b325ac9",
            "8536d7b61a1af4b78e5b7f951c0900be863c403ce65c9bfcb9382657222d18c4"
        ));
        let info = hex("4f6465206f6e2061204772656369616e2055726e");
        let aad = hex("436f756e742d30");
        let ciphertext = hex(concat!(
            "5ad590bb8baa577f8619db35a36311226a896e7342a6d836d8b7bcd2f20b6c7f",
            "9076ac232e3ab2523f39513434"
        ));
        let input = HpkeCiphertext {
            kem_output: enc.clone().into(),
            ciphertext: ciphertext.into(),
        };
        let plaintext = crypto
            .hpke_open(hpke_config(), &input, &pair.private, &info, &aad)
            .unwrap();
        assert_eq!(
            plaintext,
            hex("4265617574792069732074727574682c20747275746820626561757479")
        );

        // A.3.1.2: the exported values.
        let exported = [
            (
                hex(""),
                "5e9bc3d236e1911d95e65b576a8a86d478fb827e8bdfe77b741b289890490d4d",
            ),
            (
                hex("54657374436f6e74657874"),
                "d8f1ea7942adbba7412c6d431c62d01371ea476b823eb697e1f6e6cae1dab85a",
            ),
        ];
        for (context, value) in exported {
            let secret = crypto
                .hpke_setup_receiver_and_export(
                    hpke_config(),
                    &enc,
                    &pair.private,
                    &info,
                    &context,
                    32,
                )
                .unwrap();
            assert_eq!(secret.to_vec(), hex(value));
        }
    }

    #[test]
    fn hpke_round_trip_and_export() {
        let crypto = DaveCrypto;
        let pair = crypto.derive_hpke_keypair(hpke_config(), &[5; 32]).unwrap();

        let sealed = crypto
            .hpke_seal(hpke_config(), &pair.public, b"info", b"aad", b"secret")
            .unwrap();
        assert_eq!(sealed.kem_output.as_slice().len(), 65);
        let opened = crypto
            .hpke_open(hpke_config(), &sealed, &pair.private, b"info", b"aad")
            .unwrap();
        assert_eq!(opened, b"secret");
        assert!(
            crypto
                .hpke_open(hpke_config(), &sealed, &pair.private, b"other", b"aad")
                .is_err()
        );

        let (enc, sender_secret) = crypto
            .hpke_setup_sender_and_export(hpke_config(), &pair.public, b"info", b"context", 32)
            .unwrap();
        let receiver_secret = crypto
            .hpke_setup_receiver_and_export(
                hpke_config(),
                &enc,
                &pair.private,
                b"info",
                b"context",
                32,
            )
            .unwrap();
        assert_eq!(sender_secret.to_vec(), receiver_secret.to_vec());
        assert_eq!(sender_secret.len(), 32);
    }

    #[test]
    fn other_algorithms_are_unsupported() {
        let crypto = DaveCrypto;
        assert_eq!(
            crypto.supports(Ciphersuite::MLS_128_DHKEMX25519_AES128GCM_SHA256_Ed25519),
            Err(CryptoError::UnsupportedCiphersuite)
        );
        crypto.supports(CIPHERSUITE).unwrap();
        assert_eq!(crypto.supported_ciphersuites(), [CIPHERSUITE]);
        assert_eq!(
            crypto.hash(HashType::Sha2_384, b""),
            Err(CryptoError::UnsupportedHashAlgorithm)
        );
        assert_eq!(
            crypto.signature_key_gen(SignatureScheme::ED25519),
            Err(CryptoError::UnsupportedSignatureScheme)
        );
        assert_eq!(
            crypto
                .aead_encrypt(AeadType::Aes256Gcm, &[0; 32], b"", &[0; 12], b"")
                .err(),
            Some(CryptoError::UnsupportedAeadAlgorithm)
        );
        let config = HpkeConfig(
            HpkeKemType::DhKem25519,
            HpkeKdfType::HkdfSha256,
            HpkeAeadType::AesGcm128,
        );
        assert_eq!(
            crypto.derive_hpke_keypair(config, &[0; 32]).err(),
            Some(CryptoError::UnsupportedCiphersuite)
        );
    }

    #[test]
    fn random_values_differ() {
        let crypto = DaveCrypto;
        let first: [u8; 32] = crypto.random_array().unwrap();
        let second = crypto.random_vec(32).unwrap();
        assert_ne!(first.to_vec(), second);
    }
}
