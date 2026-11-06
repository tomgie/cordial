//! Voice UDP packets: the packet kinds, RTP, IP discovery, transport decryption, and receive.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cordial_model::id::{ChannelId, GuildId, UserId};
use cordial_voice::DaveSession;
use cordial_voice::EncryptionMode;
use cordial_voice::core::{ConnectionInfo, SharedE2ee, VoiceAction, VoiceCore};
use cordial_voice::crypto::{SecretKey, TransportCipher};
use cordial_voice::e2ee::E2eeSession;
use cordial_voice::e2ee::NoE2ee;
use cordial_voice::packet::{IpDiscovery, RTP_HEADER_LEN, RtpHeader, RtpPacket, classify};
use cordial_voice::receive::{Receiver, SsrcMap};

use crate::reader::Reader;

/// The fixed transport key.
pub const KEY: [u8; 32] = [0x42; 32];

/// The SSRC of the own user. The receiver drops its packets.
const OWN_SSRC: u32 = 3;

/// A UDP packet on the voice socket.
///
/// Selector bit 0 chooses the encryption mode. The rest goes to all packet decoders and to
/// transport decryption. Then the rest is a list of steps for a receiver: a kind byte and a
/// chunk. A step pushes the chunk as a raw packet, encrypts it with the fixed key and pushes
/// it, or runs one playout tick.
pub fn voice_packet(data: &[u8]) {
    let mut reader = Reader::new(data);
    let selector = reader.byte();
    let mode = if selector & 1 == 0 {
        EncryptionMode::Aes256GcmRtpSize
    } else {
        EncryptionMode::XChaCha20Poly1305RtpSize
    };
    let packet = reader.rest();

    decode_packet(packet);
    let cipher = TransportCipher::new(mode, &SecretKey::new(KEY));
    let _ = cipher.decrypt(packet);
    receive(cipher, reader);
}

fn decode_packet(packet: &[u8]) {
    let _ = classify(packet);

    if let Ok(header) = RtpHeader::decode(packet) {
        assert_eq!(RtpHeader::decode(&header.encode()), Ok(header));
        let _ = header.unencrypted_len();
    }
    if let Ok(parts) = RtpPacket::parse(packet) {
        assert_eq!(parts.header_bytes.len() + parts.body.len(), packet.len());
    }
    if let Ok(discovery) = IpDiscovery::decode(packet)
        && let Ok(encoded) = discovery.encode()
    {
        assert_eq!(IpDiscovery::decode(&encoded), Ok(discovery));
    }
}

fn receive(cipher: TransportCipher, mut reader: Reader<'_>) {
    let users: SsrcMap = Arc::new(Mutex::new(HashMap::from([
        (1, UserId::new_const(10)),
        (2, UserId::new_const(20)),
    ])));
    let e2ee: SharedE2ee = Arc::new(Mutex::new(Box::new(NoE2ee::default())));
    let mut receiver = Receiver::new(cipher.clone(), e2ee, users, OWN_SSRC, 3, true);

    while !reader.is_empty() {
        let kind = reader.byte();
        let chunk = reader.long_chunk();
        match kind % 4 {
            0 => receiver.push_packet(chunk),
            1 | 2 => {
                if let Some(packet) = sealed_packet(&cipher, chunk) {
                    receiver.push_packet(&packet);
                }
            }
            _ => {
                let _ = receiver.tick();
            }
        }
    }
    for _ in 0..4 {
        let _ = receiver.tick();
    }
}

/// Makes a packet that decrypts: the chunk gives the header and the payload.
fn sealed_packet(cipher: &TransportCipher, chunk: &[u8]) -> Option<Vec<u8>> {
    if chunk.len() < RTP_HEADER_LEN {
        return None;
    }
    let mut header = chunk[..RTP_HEADER_LEN].to_vec();
    // Version 2, the Opus payload type, and an SSRC from 0 to 3.
    header[0] = 0x80 | (header[0] & 0x3F);
    header[1] = (header[1] & 0x80) | 0x78;
    header[8..11].fill(0);
    header[11] &= 0x03;

    let csrc_count = usize::from(header[0] & 0x0F);
    let extension = header[0] & 0x10 != 0;
    let header_len = RTP_HEADER_LEN + 4 * csrc_count + if extension { 4 } else { 0 };
    if chunk.len() < header_len {
        return None;
    }
    header.extend_from_slice(&chunk[RTP_HEADER_LEN..header_len]);

    let nonce = u32::from(chunk[2]);
    cipher
        .encrypt_with_nonce(&header, &chunk[header_len..], nonce)
        .ok()
}

/// Frames of the voice gateway.
///
/// With selector bit 0 set, the core has a DAVE session; else it has no E2EE. The rest is a
/// list of steps: a kind byte and a chunk. A step gives the chunk to the core as a text frame
/// or a binary frame, or reports a tick, IP discovery, or a close.
pub fn voice_gateway(data: &[u8]) {
    let mut reader = Reader::new(data);
    let selector = reader.byte();
    let session: Box<dyn E2eeSession> = if selector & 1 == 0 {
        Box::new(NoE2ee::default())
    } else {
        match DaveSession::new() {
            Ok(session) => Box::new(session),
            Err(_) => return,
        }
    };
    let info = ConnectionInfo::new(
        GuildId::new_const(1),
        ChannelId::new_const(2),
        UserId::new_const(3),
        "session".to_owned(),
        "token".to_owned(),
        "voice.example.com".to_owned(),
    );
    let start = Instant::now();
    let mut core = VoiceCore::new(info, Arc::new(Mutex::new(session)));
    core.connected(start);

    let mut step = 0_u64;
    while !reader.is_empty() {
        let kind = reader.byte();
        let chunk = reader.long_chunk();
        step += 1;
        let now = start + Duration::from_secs(step);
        match kind % 6 {
            0 | 1 => {
                let _ = core.received_text(&String::from_utf8_lossy(chunk), now);
            }
            2 => {
                let _ = core.received_binary(chunk, now);
            }
            3 => core.tick(now),
            4 => core.ip_discovered(String::from_utf8_lossy(chunk).into_owned(), 50_000, now),
            _ => core.closed(
                Some(u16::from_be_bytes([kind, chunk.len().to_le_bytes()[0]])),
                now,
            ),
        }
        drain_voice_actions(&mut core, now);
    }
}

fn drain_voice_actions(core: &mut VoiceCore, now: Instant) {
    while let Some(action) = core.poll_action() {
        if let VoiceAction::Reconnect { .. } = action {
            core.connected(now);
        }
    }
}
