//! DAVE: the protocol frame codec with a fixed key, and the opcode payloads of a session.

use cordial_model::id::{ChannelId, UserId};
use cordial_voice::DaveSession;
use cordial_voice::dave::fuzzing::{FixedKeyCodec, parse_frame};
use cordial_voice::e2ee::E2eeSession;

use crate::reader::Reader;

/// The fixed base secret of the key ratchet.
pub const BASE_SECRET: [u8; 16] = [0x5A; 16];

const CHANNEL: ChannelId = ChannelId::new_const(1_234_567_890_123);
const OWN_USER: UserId = UserId::new_const(111_111_111_111_111_111);
const OTHER_USER: UserId = UserId::new_const(222_222_222_222_222_222);

/// The JSON opcodes of DAVE.
const JSON_OPS: [u8; 3] = [21, 22, 24];
/// The binary opcodes of DAVE that a client receives.
const BINARY_OPS: [u8; 4] = [25, 27, 29, 30];

/// A DAVE frame or a sequence of DAVE opcode payloads.
///
/// With selector bit 0 clear, the rest is a protocol frame for the parser and the fixed-key
/// decryptor. Then the rest is a list of Opus frames (a flip byte and a chunk): the body
/// encrypts each one, changes one byte when the flip byte is not zero, and decrypts it.
///
/// With selector bit 0 set, the rest is a list of opcode messages (an opcode byte and a
/// chunk) for a [`DaveSession`] in a call.
pub fn dave_frame(data: &[u8]) {
    let mut reader = Reader::new(data);
    let selector = reader.byte();
    if selector & 1 == 0 {
        frames(reader.rest());
    } else {
        session(selector, reader);
    }
}

fn frames(rest: &[u8]) {
    if let Some(parts) = parse_frame(rest) {
        assert_eq!(parts.rebuilt, parts.media);
    }

    let mut codec = FixedKeyCodec::new(&BASE_SECRET);
    let _ = codec.decrypt(rest);

    let mut reader = Reader::new(rest);
    while !reader.is_empty() {
        let flip = reader.byte();
        let opus = reader.chunk();
        let Some(mut frame) = codec.encrypt(opus) else {
            continue;
        };
        if flip == 0 {
            assert_eq!(codec.decrypt(&frame).as_deref(), Some(opus));
        } else {
            let index = usize::from(flip) % frame.len();
            frame[index] ^= flip;
            let _ = codec.decrypt(&frame);
        }
    }
}

fn session(selector: u8, mut reader: Reader<'_>) {
    let Ok(mut session) = DaveSession::new() else {
        return;
    };
    session.clients_connect(&[OWN_USER, OTHER_USER]);
    let protocol_version = u16::from(selector >> 1 & 1);
    let _ = session.session_description(protocol_version, OWN_USER, CHANNEL);

    while !reader.is_empty() {
        let op = reader.byte();
        let payload = reader.long_chunk();
        match op % 8 {
            0..=3 => {
                let _ = session.binary_message(BINARY_OPS[usize::from(op % 4)], payload);
            }
            4 => {
                let json_op = JSON_OPS[usize::from(op / 8) % JSON_OPS.len()];
                if let Ok(value) = serde_json::from_slice(payload) {
                    let _ = session.json_message(json_op, &value);
                }
            }
            5 => {
                let _ = session.decrypt(OTHER_USER, payload);
            }
            6 => {
                let _ = session.encrypt(payload);
            }
            _ => {
                if op & 0x80 == 0 {
                    session.client_disconnect(OTHER_USER);
                } else {
                    session.clients_connect(&[OTHER_USER]);
                }
            }
        }
    }
}
