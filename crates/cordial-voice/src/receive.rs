//! Audio receive: transport decryption, a jitter buffer for each SSRC, E2EE decryption, and
//! Opus decode with packet loss concealment.
//!
//! The [`Receiver`] has no socket and no clock. The driver gives it packets and calls
//! [`Receiver::tick`] each 20 ms.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use cordial_model::id::UserId;

#[cfg(feature = "opus")]
use crate::audio::codec::OpusDecoder;
use crate::core::{SharedE2ee, lock_e2ee};
use crate::crypto::TransportCipher;
use crate::packet::{PacketKind, RTP_PAYLOAD_TYPE, classify};

/// The default number of packets that the jitter buffer holds before playout (60 ms).
pub const DEFAULT_JITTER_DEPTH: usize = 3;
/// The maximum number of packets in a jitter buffer. Older packets are dropped.
pub const MAX_JITTER_PACKETS: usize = 50;
/// The largest gap that gives one lost step for each missing packet.
const MAX_LOST_IN_A_ROW: i32 = 10;

/// The result of one playout step of a jitter buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Playout<T> {
    /// The packet with the expected sequence number.
    Packet {
        /// The sequence number.
        sequence: u16,
        /// The packet.
        item: T,
    },
    /// The packet with the expected sequence number did not arrive.
    Lost {
        /// The sequence number.
        sequence: u16,
    },
    /// The buffer fills before playout starts.
    Waiting,
    /// The stream stopped: the buffer is empty.
    Idle,
}

/// A jitter buffer: it puts packets in sequence order and finds lost packets.
///
/// Playout starts when the buffer holds `depth` packets, or after `2 × depth` steps with
/// fewer packets. Then each [`JitterBuffer::pop`] gives the next sequence number: the
/// packet, or `Lost` when it is missing and later packets wait. When the buffer is empty,
/// the stream is idle and must fill again.
#[derive(Clone, Debug)]
pub struct JitterBuffer<T> {
    packets: HashMap<u16, T>,
    next: Option<u16>,
    depth: usize,
    /// The number of steps that the buffer waited with packets but before playout.
    waited: usize,
}

impl<T> JitterBuffer<T> {
    /// Makes a buffer that starts playout at `depth` packets (at least 1).
    #[must_use]
    pub fn new(depth: usize) -> Self {
        Self {
            packets: HashMap::new(),
            next: None,
            depth: depth.max(1),
            waited: 0,
        }
    }

    /// Gives the number of packets in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.packets.len()
    }

    /// Tells if the buffer has no packets.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    /// Adds a packet. Gives `false` when the packet is late or a duplicate and is dropped.
    pub fn push(&mut self, sequence: u16, item: T) -> bool {
        if let Some(next) = self.next
            && distance(next, sequence) < 0
        {
            return false;
        }
        if self.packets.contains_key(&sequence) {
            return false;
        }
        self.packets.insert(sequence, item);

        if self.packets.len() > MAX_JITTER_PACKETS {
            // The consumer is too slow: keep the newest half and start the playout again.
            let mut keys: Vec<u16> = self.packets.keys().copied().collect();
            keys.sort_by_key(|key| distance(sequence, *key));
            let remove = keys.len() - MAX_JITTER_PACKETS / 2;
            for key in &keys[..remove] {
                self.packets.remove(key);
            }
            self.next = None;
        }
        true
    }

    /// Gives the next playout step.
    pub fn pop(&mut self) -> Playout<T> {
        let next = match self.next {
            Some(next) => next,
            None => {
                let Some(oldest) = self.oldest() else {
                    self.waited = 0;
                    return Playout::Waiting;
                };
                // A sender that stops before the buffer is full must not leave old packets
                // that play much later, before its next audio.
                if self.packets.len() < self.depth && self.waited < self.depth * 2 {
                    self.waited += 1;
                    return Playout::Waiting;
                }
                self.waited = 0;
                oldest
            }
        };

        if self.packets.is_empty() {
            self.next = None;
            return Playout::Idle;
        }
        self.next = Some(next.wrapping_add(1));
        if let Some(item) = self.packets.remove(&next) {
            return Playout::Packet {
                sequence: next,
                item,
            };
        }

        // After a large jump, continue at the oldest packet instead of many lost steps.
        if let Some(oldest) = self.oldest()
            && distance(next, oldest) > MAX_LOST_IN_A_ROW
        {
            self.next = Some(oldest);
        }
        Playout::Lost { sequence: next }
    }

    fn oldest(&self) -> Option<u16> {
        let reference = *self.packets.keys().next()?;
        self.packets
            .keys()
            .copied()
            .min_by_key(|key| distance(reference, *key))
    }
}

/// The signed distance from `from` to `to` with wrap-around.
fn distance(from: u16, to: u16) -> i32 {
    let difference = to.wrapping_sub(from);
    i32::from(i16::from_be_bytes(difference.to_be_bytes()))
}

/// 20 ms of audio from one sender.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct VoiceData {
    /// The SSRC of the sender.
    pub ssrc: u32,
    /// The user of the SSRC, when the voice gateway told it (opcode 5).
    pub user_id: Option<UserId>,
    /// The RTP sequence number.
    pub sequence: u16,
    /// The RTP timestamp. `None` for a lost packet.
    pub timestamp: Option<u32>,
    /// The Opus packet after all decryption. `None` for a lost packet.
    pub opus: Option<Vec<u8>>,
    /// Decoded interleaved stereo PCM at 48 kHz. Needs the `opus` feature. For a lost packet
    /// it is the output of packet loss concealment.
    pub pcm: Option<Vec<i16>>,
}

impl VoiceData {
    /// Tells if the packet was lost.
    #[must_use]
    pub fn is_lost(&self) -> bool {
        self.opus.is_none()
    }
}

/// The SSRC-to-user map of a call.
pub type SsrcMap = Arc<Mutex<HashMap<u32, UserId>>>;

#[derive(Debug)]
struct Received {
    timestamp: u32,
    payload: Vec<u8>,
}

struct Stream {
    buffer: JitterBuffer<Received>,
    #[cfg(feature = "opus")]
    decoder: Option<OpusDecoder>,
}

/// The receive pipeline of a call.
pub struct Receiver {
    cipher: TransportCipher,
    e2ee: SharedE2ee,
    users: SsrcMap,
    own_ssrc: u32,
    depth: usize,
    decode: bool,
    streams: HashMap<u32, Stream>,
}

impl fmt::Debug for Receiver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Receiver")
            .field("own_ssrc", &self.own_ssrc)
            .field("streams", &self.streams.len())
            .finish_non_exhaustive()
    }
}

impl Receiver {
    /// Makes a receiver.
    ///
    /// `decode` turns Opus decode on; it has an effect only with the `opus` feature.
    #[must_use]
    pub fn new(
        cipher: TransportCipher,
        e2ee: SharedE2ee,
        users: SsrcMap,
        own_ssrc: u32,
        depth: usize,
        decode: bool,
    ) -> Self {
        Self {
            cipher,
            e2ee,
            users,
            own_ssrc,
            depth,
            decode,
            streams: HashMap::new(),
        }
    }

    /// Handles a UDP packet. Drops RTCP, keepalive, discovery, and packets that cannot be
    /// decrypted.
    pub fn push_packet(&mut self, packet: &[u8]) {
        if classify(packet) != PacketKind::Rtp {
            return;
        }
        let decrypted = match self.cipher.decrypt(packet) {
            Ok(decrypted) => decrypted,
            Err(error) => {
                tracing::trace!(%error, "dropped a voice packet");
                return;
            }
        };
        let header = decrypted.header;
        if header.payload_type != RTP_PAYLOAD_TYPE || header.ssrc == self.own_ssrc {
            return;
        }

        let depth = self.depth;
        let stream = self.streams.entry(header.ssrc).or_insert_with(|| Stream {
            buffer: JitterBuffer::new(depth),
            #[cfg(feature = "opus")]
            decoder: None,
        });
        stream.buffer.push(
            header.sequence,
            Received {
                timestamp: header.timestamp,
                payload: decrypted.payload,
            },
        );
    }

    /// Forgets the stream of an SSRC, for example after the user left.
    pub fn remove_ssrc(&mut self, ssrc: u32) {
        self.streams.remove(&ssrc);
    }

    /// Runs one 20 ms playout step for all streams.
    pub fn tick(&mut self) -> Vec<VoiceData> {
        let users = self
            .users
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let mut output = Vec::new();
        let mut idle = Vec::new();

        for (&ssrc, stream) in &mut self.streams {
            let user_id = users.get(&ssrc).copied();
            match stream.buffer.pop() {
                Playout::Packet { sequence, item } => {
                    let Some(opus) = e2ee_decrypt(&self.e2ee, user_id, item.payload) else {
                        continue;
                    };
                    let pcm = if self.decode {
                        decode_packet(stream, Some(&opus))
                    } else {
                        None
                    };
                    output.push(VoiceData {
                        ssrc,
                        user_id,
                        sequence,
                        timestamp: Some(item.timestamp),
                        opus: Some(opus),
                        pcm,
                    });
                }
                Playout::Lost { sequence } => {
                    let pcm = if self.decode {
                        decode_packet(stream, None)
                    } else {
                        None
                    };
                    output.push(VoiceData {
                        ssrc,
                        user_id,
                        sequence,
                        timestamp: None,
                        opus: None,
                        pcm,
                    });
                }
                Playout::Idle => idle.push(ssrc),
                Playout::Waiting => {}
            }
        }

        // An idle stream gets a new jitter buffer and decoder at its next packet.
        for ssrc in idle {
            self.streams.remove(&ssrc);
        }
        output.sort_by_key(|data| data.ssrc);
        output
    }
}

/// Decrypts the DAVE layer. Gives `None` when the frame must be dropped.
fn e2ee_decrypt(e2ee: &SharedE2ee, user_id: Option<UserId>, payload: Vec<u8>) -> Option<Vec<u8>> {
    let Some(user_id) = user_id else {
        // Without a user, a DAVE frame (marker 0xFAFA) cannot be decrypted.
        if payload.ends_with(&[0xFA, 0xFA]) {
            return None;
        }
        return Some(payload);
    };
    match lock_e2ee(e2ee).decrypt(user_id, &payload) {
        Ok(frame) => Some(frame),
        Err(error) => {
            tracing::debug!(%error, user = %user_id, "cannot decrypt an E2EE frame");
            None
        }
    }
}

#[cfg(feature = "opus")]
fn decode_packet(stream: &mut Stream, packet: Option<&[u8]>) -> Option<Vec<i16>> {
    if stream.decoder.is_none() {
        stream.decoder = OpusDecoder::new().ok();
    }
    let decoder = stream.decoder.as_mut()?;
    let result = match packet {
        Some(packet) => decoder.decode(packet),
        None => decoder.conceal(),
    };
    result
        .map_err(|error| tracing::debug!(%error, "cannot decode a received Opus packet"))
        .ok()
}

#[cfg(not(feature = "opus"))]
fn decode_packet(_stream: &mut Stream, _packet: Option<&[u8]>) -> Option<Vec<i16>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::SecretKey;
    use crate::e2ee::NoE2ee;
    use crate::packet::RtpHeader;
    use crate::protocol::EncryptionMode;

    fn sequence_of(playout: &Playout<&str>) -> String {
        match playout {
            Playout::Packet { sequence, item } => format!("{sequence}:{item}"),
            Playout::Lost { sequence } => format!("{sequence}:lost"),
            Playout::Waiting => "waiting".to_owned(),
            Playout::Idle => "idle".to_owned(),
        }
    }

    #[test]
    fn reorders_and_finds_gaps() {
        let mut buffer = JitterBuffer::new(3);
        assert!(buffer.push(11, "b"));
        assert_eq!(buffer.pop(), Playout::Waiting);
        assert!(buffer.push(10, "a"));
        assert!(buffer.push(13, "d"));
        assert!(!buffer.push(13, "d"));
        let steps: Vec<String> = (0..5).map(|_| sequence_of(&buffer.pop())).collect();
        assert_eq!(steps, vec!["10:a", "11:b", "12:lost", "13:d", "idle"]);
        assert_eq!(buffer.pop(), Playout::Waiting);
    }

    #[test]
    fn playout_starts_after_a_wait_with_few_packets() {
        let mut buffer = JitterBuffer::new(3);
        assert_eq!(buffer.pop(), Playout::Waiting);
        buffer.push(10, "a");
        let steps: Vec<String> = (0..8).map(|_| sequence_of(&buffer.pop())).collect();
        assert_eq!(
            steps,
            vec![
                "waiting", "waiting", "waiting", "waiting", "waiting", "waiting", "10:a", "idle"
            ]
        );
    }

    #[test]
    fn drops_late_packets() {
        let mut buffer = JitterBuffer::new(1);
        buffer.push(5, "a");
        buffer.push(6, "b");
        assert_eq!(sequence_of(&buffer.pop()), "5:a");
        assert!(!buffer.push(5, "late"));
        assert!(!buffer.push(4, "older"));
        assert!(buffer.push(7, "c"));
        assert_eq!(sequence_of(&buffer.pop()), "6:b");
        assert_eq!(sequence_of(&buffer.pop()), "7:c");
    }

    #[test]
    fn wraps_around() {
        let mut buffer = JitterBuffer::new(3);
        buffer.push(1, "c");
        buffer.push(65_535, "a");
        buffer.push(0, "b");
        let steps: Vec<String> = (0..4).map(|_| sequence_of(&buffer.pop())).collect();
        assert_eq!(steps, vec!["65535:a", "0:b", "1:c", "idle"]);
    }

    #[test]
    fn overflow_starts_again() {
        let mut buffer = JitterBuffer::new(1);
        for sequence in 0..=u16::try_from(MAX_JITTER_PACKETS).expect("small") {
            buffer.push(sequence, "x");
        }
        assert_eq!(buffer.len(), MAX_JITTER_PACKETS / 2);
        assert_eq!(sequence_of(&buffer.pop()), "26:x");
    }

    #[test]
    fn large_gap_skips_ahead() {
        let mut buffer = JitterBuffer::new(1);
        buffer.push(1, "a");
        assert_eq!(sequence_of(&buffer.pop()), "1:a");
        buffer.push(500, "b");
        assert_eq!(sequence_of(&buffer.pop()), "2:lost");
        assert_eq!(sequence_of(&buffer.pop()), "500:b");
    }

    fn packet(cipher: &mut TransportCipher, sequence: u16, ssrc: u32, payload: &[u8]) -> Vec<u8> {
        let header = RtpHeader::new(sequence, u32::from(sequence) * 960, ssrc).encode();
        cipher.encrypt(&header, payload).expect("encrypt")
    }

    #[test]
    fn pipeline() {
        let key = SecretKey::new([3; 32]);
        let mode = EncryptionMode::XChaCha20Poly1305RtpSize;
        let mut sender = TransportCipher::new(mode, &key);
        let users: SsrcMap = Arc::default();
        users
            .lock()
            .expect("lock")
            .insert(20, UserId::new_const(99));
        let mut receiver = Receiver::new(
            TransportCipher::new(mode, &key),
            Arc::new(Mutex::new(Box::new(NoE2ee))),
            Arc::clone(&users),
            1,
            2,
            true,
        );

        receiver.push_packet(&packet(&mut sender, 1, 20, &[0xF8, 0xFF, 0xFE]));
        // RTCP, the own SSRC, a bad tag, and a keepalive are dropped.
        let mut rtcp = packet(&mut sender, 2, 20, &[1]);
        rtcp[1] = 200;
        receiver.push_packet(&rtcp);
        receiver.push_packet(&packet(&mut sender, 5, 1, &[1]));
        let mut broken = packet(&mut sender, 6, 20, &[1]);
        broken[13] ^= 1;
        receiver.push_packet(&broken);
        receiver.push_packet(&[0x13, 0x37, 0xCA, 0xFE, 0, 0, 0, 0]);
        assert!(receiver.tick().is_empty());

        receiver.push_packet(&packet(&mut sender, 3, 20, &[0xF8, 0xFF, 0xFE]));
        receiver.push_packet(&packet(&mut sender, 9, 30, &[0xF8, 0xFF, 0xFE]));
        let first = receiver.tick();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].ssrc, 20);
        assert_eq!(first[0].user_id, Some(UserId::new_const(99)));
        assert_eq!(first[0].sequence, 1);
        assert_eq!(first[0].timestamp, Some(960));
        assert_eq!(first[0].opus.as_deref(), Some(&[0xF8, 0xFF, 0xFE][..]));
        assert_eq!(
            first[0].pcm.as_ref().map(Vec::len),
            cfg!(feature = "opus").then_some(crate::audio::FRAME_LEN)
        );

        let second = receiver.tick();
        assert!(second[0].is_lost());
        assert_eq!(second[0].sequence, 2);
        assert_eq!(
            second[0].pcm.as_ref().map(Vec::len),
            cfg!(feature = "opus").then_some(crate::audio::FRAME_LEN)
        );

        let third = receiver.tick();
        assert_eq!(third[0].sequence, 3);
        assert!(receiver.tick().is_empty());

        // A DAVE frame from an unknown SSRC is dropped.
        receiver.push_packet(&packet(&mut sender, 10, 30, &[1, 2, 0xFA, 0xFA]));
        let data = receiver.tick();
        assert_eq!(data.len(), 1);
        assert_eq!(data[0].ssrc, 30);
        assert_eq!(data[0].user_id, None);
        assert!(receiver.tick().is_empty());
    }
}
