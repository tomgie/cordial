//! The receive side: one decryptor for each sender.
//!
//! The logic follows libdave `decryptor.cpp` and `cryptor_manager.cpp`. A decryptor keeps one
//! cryptor manager for each key ratchet (one epoch of one sender). After a transition, the
//! managers of earlier epochs stay for 10 seconds so that frames in flight still decrypt.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use super::cryptor::Cryptor;
use super::frame::{self, OPUS_SILENCE_FRAME, ProtocolFrame};
use super::ratchet::KeyRatchet;

/// The time that old keys stay usable after a transition (libdave `kCryptorExpiry`,
/// `kDefaultTransitionDuration`).
pub(crate) const TRANSITION_EXPIRY: Duration = Duration::from_secs(10);

/// The largest accepted jump of the generation (libdave `kMaxGenerationGap`).
const MAX_GENERATION_GAP: u32 = 250;

/// The number of skipped nonces that stay acceptable (libdave `kMaxMissingNonces`).
const MAX_MISSING_NONCES: u64 = 1000;

/// The number of generations before the generation byte wraps (libdave `kGenerationWrap`).
const GENERATION_WRAP: u32 = 256;

/// The shift from the truncated nonce to the generation (libdave `kRatchetGenerationShiftBits`).
pub(crate) const GENERATION_SHIFT: u32 = 24;

/// The highest frame rate that libdave expects: 50 audio frames and two 60 fps video streams
/// (libdave `kMaxFramesPerSecond`).
const MAX_FRAMES_PER_SECOND: u64 = 50 + 2 * 60;

/// Gives the full generation from the generation byte of a nonce (libdave
/// `ComputeWrappedGeneration`). The function assumes that `generation` is not older than
/// `oldest`.
pub(crate) fn wrapped_generation(oldest: u32, generation: u32) -> u32 {
    let remainder = oldest % GENERATION_WRAP;
    let factor = oldest / GENERATION_WRAP + u32::from(generation < remainder);
    factor
        .wrapping_mul(GENERATION_WRAP)
        .wrapping_add(generation)
}

/// Gives the 64-bit nonce from the full generation and the truncated nonce (libdave
/// `ComputeWrappedBigNonce`).
fn wrapped_big_nonce(generation: u32, nonce: u32) -> u64 {
    let masked_nonce = u64::from(nonce & ((1 << GENERATION_SHIFT) - 1));
    (u64::from(generation) << GENERATION_SHIFT) | masked_nonce
}

/// Why a frame does not decrypt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecryptFailure {
    /// The frame is not a protocol frame and passthrough is off.
    NotEncrypted,
    /// No key ratchet is available.
    MissingKey,
    /// The nonce was used before or is too old.
    InvalidNonce,
    /// The generation is not acceptable.
    InvalidGeneration,
    /// The tag is not correct.
    Authentication,
}

impl DecryptFailure {
    /// Gives a short description.
    pub(crate) fn describe(self) -> &'static str {
        match self {
            Self::NotEncrypted => "the frame is not encrypted and passthrough is off",
            Self::MissingKey => "no key is available for the sender",
            Self::InvalidNonce => "the nonce was used before or is too old",
            Self::InvalidGeneration => "the key generation is not acceptable",
            Self::Authentication => "the authentication tag is not correct",
        }
    }
}

#[derive(Debug)]
struct ExpiringCryptor {
    cryptor: Cryptor,
    expiry: Option<Instant>,
}

/// The cryptors and the replay state of one key ratchet.
#[derive(Debug)]
pub(crate) struct CryptorManager {
    ratchet: KeyRatchet,
    created: Instant,
    expiry: Option<Instant>,
    cryptors: HashMap<u32, ExpiringCryptor>,
    oldest_generation: u32,
    newest_generation: u32,
    newest_processed_nonce: Option<u64>,
    missing_nonces: VecDeque<u64>,
}

impl CryptorManager {
    pub(crate) fn new(ratchet: KeyRatchet, now: Instant) -> Self {
        Self {
            ratchet,
            created: now,
            expiry: None,
            cryptors: HashMap::new(),
            oldest_generation: 0,
            newest_generation: 0,
            newest_processed_nonce: None,
            missing_nonces: VecDeque::new(),
        }
    }

    fn update_expiry(&mut self, expiry: Instant) {
        self.expiry = Some(self.expiry.map_or(expiry, |current| current.min(expiry)));
    }

    fn is_expired(&self, now: Instant) -> bool {
        self.expiry.is_some_and(|expiry| now > expiry)
    }

    fn can_process_nonce(&self, generation: u32, nonce: u32) -> bool {
        let big_nonce = wrapped_big_nonce(generation, nonce);
        let expected = self.newest_processed_nonce.map_or(0, |newest| newest + 1);
        big_nonce >= expected
            || self
                .missing_nonces
                .iter()
                .rev()
                .any(|&missing| missing == big_nonce)
    }

    fn cryptor(&mut self, generation: u32, now: Instant) -> Option<&Cryptor> {
        self.cleanup_expired_cryptors(now);

        if generation < self.oldest_generation {
            return None;
        }
        if generation > self.newest_generation.saturating_add(MAX_GENERATION_GAP) {
            return None;
        }

        // A sender cannot reach a generation faster than the highest frame rate allows.
        let lifetime_seconds = now.saturating_duration_since(self.created).as_secs();
        let max_lifetime_frames = MAX_FRAMES_PER_SECOND.saturating_mul(lifetime_seconds);
        let max_lifetime_generations = max_lifetime_frames >> GENERATION_SHIFT;
        if u64::from(generation) > max_lifetime_generations {
            return None;
        }

        if !self.cryptors.contains_key(&generation) {
            let key = self.ratchet.get(generation)?;
            // A cryptor for a generation older than the newest expires soon, because the
            // sender already moved on.
            let expiry = (generation < self.newest_generation).then(|| now + TRANSITION_EXPIRY);
            self.cryptors.insert(
                generation,
                ExpiringCryptor {
                    cryptor: Cryptor::new(&key),
                    expiry,
                },
            );
        }
        self.cryptors.get(&generation).map(|entry| &entry.cryptor)
    }

    fn report_success(&mut self, generation: u32, nonce: u32, now: Instant) {
        let big_nonce = wrapped_big_nonce(generation, nonce);
        let expected = self.newest_processed_nonce.map_or(0, |newest| newest + 1);

        if big_nonce >= expected {
            let missing = (big_nonce - expected).min(MAX_MISSING_NONCES);
            while !self.missing_nonces.is_empty()
                && self.missing_nonces.len() as u64 + missing > MAX_MISSING_NONCES
            {
                self.missing_nonces.pop_front();
            }
            self.missing_nonces.extend(big_nonce - missing..big_nonce);
            self.newest_processed_nonce = Some(big_nonce);
        } else if let Some(position) = self
            .missing_nonces
            .iter()
            .rposition(|&value| value == big_nonce)
        {
            self.missing_nonces.remove(position);
        }

        if generation <= self.newest_generation || !self.cryptors.contains_key(&generation) {
            return;
        }
        self.newest_generation = generation;

        let expiry = now + TRANSITION_EXPIRY;
        for (&cryptor_generation, entry) in &mut self.cryptors {
            if cryptor_generation < generation {
                entry.expiry = Some(entry.expiry.map_or(expiry, |current| current.min(expiry)));
            }
        }
    }

    fn cleanup_expired_cryptors(&mut self, now: Instant) {
        self.cryptors
            .retain(|_, entry| entry.expiry.is_none_or(|expiry| expiry >= now));

        while self.oldest_generation < self.newest_generation
            && !self.cryptors.contains_key(&self.oldest_generation)
        {
            self.ratchet.erase(self.oldest_generation);
            self.oldest_generation += 1;
        }
    }

    fn decrypt(
        &mut self,
        frame: &ProtocolFrame<'_>,
        now: Instant,
    ) -> Result<Vec<u8>, DecryptFailure> {
        let generation =
            wrapped_generation(self.oldest_generation, frame.nonce >> GENERATION_SHIFT);
        if !self.can_process_nonce(generation, frame.nonce) {
            return Err(DecryptFailure::InvalidNonce);
        }

        let (authenticated, ciphertext) = frame.split();
        let cryptor = self
            .cryptor(generation, now)
            .ok_or(DecryptFailure::InvalidGeneration)?;
        let plaintext = cryptor
            .decrypt(frame.nonce, &authenticated, &ciphertext, &frame.tag)
            .ok_or(DecryptFailure::Authentication)?;

        self.report_success(generation, frame.nonce, now);
        Ok(frame.reconstruct(&plaintext))
    }
}

/// When unencrypted frames pass through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PassthroughWindow {
    Never,
    Until(Instant),
    Always,
}

/// The decryptor of one sender.
#[derive(Debug)]
pub(crate) struct Decryptor {
    managers: VecDeque<CryptorManager>,
    passthrough: PassthroughWindow,
}

impl Decryptor {
    /// Makes a decryptor. `passthrough` sets whether unencrypted frames pass through.
    pub(crate) fn new(passthrough: bool) -> Self {
        Self {
            managers: VecDeque::new(),
            passthrough: if passthrough {
                PassthroughWindow::Always
            } else {
                PassthroughWindow::Never
            },
        }
    }

    /// Adds the key ratchet of a new epoch. The earlier ratchets expire after the transition
    /// time. A ratchet of a key domain that is already present has no effect, so that a replay
    /// does not get a new replay window (libdave `Decryptor::TransitionToKeyRatchet`).
    pub(crate) fn transition_to_key_ratchet(&mut self, ratchet: Option<KeyRatchet>, now: Instant) {
        self.cleanup_expired_managers(now);

        if let Some(ratchet) = &ratchet
            && self
                .managers
                .iter()
                .any(|manager| manager.ratchet.domain() == ratchet.domain())
        {
            return;
        }

        let expiry = now + TRANSITION_EXPIRY;
        for manager in &mut self.managers {
            manager.update_expiry(expiry);
        }
        if let Some(ratchet) = ratchet {
            self.managers.push_back(CryptorManager::new(ratchet, now));
        }
    }

    /// Turns passthrough on, or turns it off after the transition time (libdave
    /// `Decryptor::TransitionToPassthroughMode`).
    pub(crate) fn transition_to_passthrough(&mut self, passthrough: bool, now: Instant) {
        if passthrough {
            self.passthrough = PassthroughWindow::Always;
            return;
        }

        let limit = now + TRANSITION_EXPIRY;
        self.passthrough = match self.passthrough {
            PassthroughWindow::Always => PassthroughWindow::Until(limit),
            PassthroughWindow::Until(until) => PassthroughWindow::Until(until.min(limit)),
            PassthroughWindow::Never => PassthroughWindow::Never,
        };
    }

    fn passthrough_allowed(&self, now: Instant) -> bool {
        match self.passthrough {
            PassthroughWindow::Never => false,
            PassthroughWindow::Until(until) => until > now,
            PassthroughWindow::Always => true,
        }
    }

    fn cleanup_expired_managers(&mut self, now: Instant) {
        while self
            .managers
            .front()
            .is_some_and(|manager| manager.is_expired(now))
        {
            self.managers.pop_front();
        }
    }

    /// Decrypts a frame.
    pub(crate) fn decrypt(
        &mut self,
        encrypted: &[u8],
        now: Instant,
    ) -> Result<Vec<u8>, DecryptFailure> {
        if encrypted == OPUS_SILENCE_FRAME {
            return Ok(encrypted.to_vec());
        }

        self.cleanup_expired_managers(now);

        let Some(parsed) = frame::parse(encrypted) else {
            if self.passthrough_allowed(now) {
                return Ok(encrypted.to_vec());
            }
            return Err(DecryptFailure::NotEncrypted);
        };

        // Try the newest ratchet first.
        let mut failure = DecryptFailure::MissingKey;
        for manager in self.managers.iter_mut().rev() {
            match manager.decrypt(&parsed, now) {
                Ok(plaintext) => return Ok(plaintext),
                Err(error) => failure = error,
            }
        }
        Err(failure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dave::encryptor::Encryptor;

    fn ratchet(fill: u8) -> KeyRatchet {
        KeyRatchet::new(vec![fill; 16])
    }

    fn encryptor(fill: u8) -> Encryptor {
        let mut encryptor = Encryptor::new();
        encryptor.set_passthrough(false);
        encryptor.set_key_ratchet(Some(ratchet(fill)));
        encryptor
    }

    #[test]
    fn wrapped_generation_values() {
        assert_eq!(wrapped_generation(0, 0), 0);
        assert_eq!(wrapped_generation(0, 5), 5);
        assert_eq!(wrapped_generation(250, 3), 259);
        assert_eq!(wrapped_generation(250, 251), 251);
        assert_eq!(wrapped_generation(256, 0), 256);
        assert_eq!(wrapped_generation(300, 50), 306);
    }

    #[test]
    fn round_trip_and_replay_rejection() {
        let now = Instant::now();
        let mut sender = encryptor(1);
        let mut receiver = Decryptor::new(false);
        receiver.transition_to_key_ratchet(Some(ratchet(1)), now);

        let frame = sender.encrypt(b"opus frame").unwrap();
        assert_eq!(receiver.decrypt(&frame, now).unwrap(), b"opus frame");
        assert_eq!(
            receiver.decrypt(&frame, now),
            Err(DecryptFailure::InvalidNonce)
        );

        // A reinstall of the same key domain does not reset the replay state.
        receiver.transition_to_key_ratchet(Some(ratchet(1)), now);
        assert_eq!(
            receiver.decrypt(&frame, now),
            Err(DecryptFailure::InvalidNonce)
        );
    }

    #[test]
    fn out_of_order_frames() {
        let now = Instant::now();
        let mut sender = encryptor(2);
        let mut receiver = Decryptor::new(false);
        receiver.transition_to_key_ratchet(Some(ratchet(2)), now);

        let first = sender.encrypt(b"one").unwrap();
        let second = sender.encrypt(b"two").unwrap();
        let third = sender.encrypt(b"three").unwrap();

        assert_eq!(receiver.decrypt(&third, now).unwrap(), b"three");
        assert_eq!(receiver.decrypt(&first, now).unwrap(), b"one");
        assert_eq!(receiver.decrypt(&second, now).unwrap(), b"two");
        assert!(receiver.decrypt(&first, now).is_err());
        assert!(receiver.decrypt(&second, now).is_err());
    }

    #[test]
    fn tampered_frame_fails_and_original_still_decrypts() {
        let now = Instant::now();
        let mut sender = encryptor(3);
        let mut receiver = Decryptor::new(false);
        receiver.transition_to_key_ratchet(Some(ratchet(3)), now);

        let frame = sender.encrypt(&[0x55; 40]).unwrap();
        let mut tampered = frame.clone();
        tampered[10] ^= 1;
        assert_eq!(
            receiver.decrypt(&tampered, now),
            Err(DecryptFailure::Authentication)
        );
        assert_eq!(receiver.decrypt(&frame, now).unwrap(), [0x55; 40]);
    }

    #[test]
    fn wrong_key_fails() {
        let now = Instant::now();
        let mut sender = encryptor(4);
        let mut receiver = Decryptor::new(false);
        receiver.transition_to_key_ratchet(Some(ratchet(5)), now);
        assert!(
            receiver
                .decrypt(&sender.encrypt(b"x").unwrap(), now)
                .is_err()
        );
    }

    #[test]
    fn silence_and_passthrough() {
        let now = Instant::now();
        let mut receiver = Decryptor::new(false);
        assert_eq!(
            receiver.decrypt(&OPUS_SILENCE_FRAME, now).unwrap(),
            OPUS_SILENCE_FRAME
        );
        assert_eq!(
            receiver.decrypt(b"plain", now),
            Err(DecryptFailure::NotEncrypted)
        );

        receiver.transition_to_passthrough(true, now);
        assert_eq!(receiver.decrypt(b"plain", now).unwrap(), b"plain");

        receiver.transition_to_passthrough(false, now);
        assert_eq!(receiver.decrypt(b"plain", now).unwrap(), b"plain");
        let later = now + TRANSITION_EXPIRY + Duration::from_secs(1);
        assert_eq!(
            receiver.decrypt(b"plain", later),
            Err(DecryptFailure::NotEncrypted)
        );
    }

    #[test]
    fn old_epoch_expires_after_transition() {
        let now = Instant::now();
        let mut old_sender = encryptor(6);
        let mut new_sender = encryptor(7);
        let mut receiver = Decryptor::new(false);
        receiver.transition_to_key_ratchet(Some(ratchet(6)), now);

        let old_first = old_sender.encrypt(b"old 1").unwrap();
        let old_second = old_sender.encrypt(b"old 2").unwrap();
        receiver.transition_to_key_ratchet(Some(ratchet(7)), now);

        let new_frame = new_sender.encrypt(b"new").unwrap();
        assert_eq!(receiver.decrypt(&new_frame, now).unwrap(), b"new");
        let soon = now + Duration::from_secs(5);
        assert_eq!(receiver.decrypt(&old_first, soon).unwrap(), b"old 1");
        let late = now + Duration::from_secs(11);
        assert!(receiver.decrypt(&old_second, late).is_err());
    }

    #[test]
    fn generation_gap_and_lifetime_limits() {
        let now = Instant::now();
        let mut manager = CryptorManager::new(ratchet(8), now);
        // A young ratchet accepts generation 0 only.
        assert!(manager.cryptor(0, now).is_some());
        assert!(manager.cryptor(1, now).is_none());

        // After a long time, generations up to the gap are acceptable.
        let later = now + Duration::from_secs(200_000_000);
        assert!(manager.cryptor(MAX_GENERATION_GAP, later).is_some());
        assert!(manager.cryptor(MAX_GENERATION_GAP + 1, later).is_none());
    }

    #[test]
    fn missing_nonce_window() {
        let now = Instant::now();
        let mut manager = CryptorManager::new(ratchet(9), now);
        manager.report_success(0, 1, now);
        manager.report_success(0, 2000, now);
        // Nonce 2 is more than 1000 behind, so it is not acceptable.
        assert!(!manager.can_process_nonce(0, 2));
        assert!(manager.can_process_nonce(0, 1500));
        assert!(!manager.can_process_nonce(0, 2000));
        assert!(manager.can_process_nonce(0, 2001));
        assert_eq!(manager.missing_nonces.len() as u64, MAX_MISSING_NONCES);
    }
}
