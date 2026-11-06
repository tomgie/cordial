//! Hand-made seed inputs that reach deep paths of each body.
//!
//! The smoke test mutates them, and `write_corpus` gives them to libFuzzer.

use cordial_voice::EncryptionMode;
use cordial_voice::audio::ogg::OggOpusWriter;
use cordial_voice::crypto::{SecretKey, TransportCipher};
use cordial_voice::dave::fuzzing::FixedKeyCodec;
use cordial_voice::packet::{IpDiscovery, RtpHeader};

use crate::dave::BASE_SECRET;
use crate::gateway::{zlib_frames, zstd_frames};
use crate::voice::KEY;

const GATEWAY_SESSION: &str = concat!(
    r#"{"op":10,"d":{"heartbeat_interval":41250}}"#,
    "\n",
    r#"{"op":0,"s":1,"t":"READY","d":{"v":10,"user":{"id":"99","username":"bot","discriminator":"0","avatar":null},"guilds":[{"id":"100","unavailable":true}],"session_id":"s","resume_gateway_url":"wss://x","application":{"id":"99","flags":0}}}"#,
    "\n",
    r#"{"op":11}"#,
    "\n",
    r#"{"op":0,"s":2,"t":"CHANNEL_CREATE","d":{"id":"10","type":0,"guild_id":"100","name":"general","position":0,"permission_overwrites":[]}}"#,
    "\n",
    r#"{"op":1,"d":null}"#,
    "\n",
    r#"{"op":9,"d":true}"#,
    "\n",
    r#"{"op":7,"d":null}"#,
);

const MESSAGE: &str = r#"{"id":"5678","channel_id":"1234","content":"Hello","timestamp":"2026-01-01T00:00:00+00:00","edited_timestamp":null,"tts":false,"mention_everyone":false,"mentions":[],"mention_roles":[],"attachments":[],"embeds":[],"pinned":false,"type":0,"author":{"id":"9","username":"bot","discriminator":"0","global_name":null,"avatar":null}}"#;

const ERROR_BODY: &str = r#"{"code":50035,"errors":{"embeds":{"0":{"title":{"_errors":[{"code":"BASE_TYPE_MAX_LENGTH","message":"Must be 256 or fewer in length."}]}}}},"message":"Invalid Form Body"}"#;

const RATE_LIMIT_BODY: &str =
    r#"{"message":"You are being rate limited.","retry_after":0.5,"global":false}"#;

const COMMAND_DATA: &str = r#"{"id":"1","name":"fuzz","type":1,"options":[{"name":"user","type":6,"value":"5"},{"name":"text","type":3,"value":"hi","focused":true},{"name":"integer","type":4,"value":300},{"name":"group","type":2,"options":[{"name":"sub","type":1,"options":[{"name":"number","type":10,"value":1.5}]}]}],"resolved":{"users":{"5":{"id":"5","username":"user","discriminator":"0","avatar":null}},"members":{"5":{"roles":[],"joined_at":"2026-01-01T00:00:00+00:00","flags":0}}}}"#;

const PING: &str = r#"{"id":"1","application_id":"2","type":1,"token":"t","version":1}"#;

const WEBHOOK_EVENT: &str = r#"{"version":1,"application_id":"2","type":1,"event":{"type":"APPLICATION_AUTHORIZED","timestamp":"2026-01-01T00:00:00.000000+00:00","data":{"integration_type":0,"scopes":["applications.commands"],"user":{"id":"5","username":"user","discriminator":"0","avatar":null}}}}"#;

/// Gives the seeds of a target. Each seed starts with the selector byte.
#[must_use]
pub fn for_target(name: &str) -> Vec<Vec<u8>> {
    match name {
        "gateway_payload" => gateway_payload(),
        "gateway_compression" => gateway_compression(),
        "http_response" => http_response(),
        "voice_packet" => voice_packet(),
        "voice_gateway" => voice_gateway(),
        "dave_frame" => dave_frame(),
        "ogg" => ogg(),
        "endpoint" => endpoint(),
        "command_parse" => command_parse(),
        _ => Vec::new(),
    }
}

fn with_selector(selector: u8, body: &[u8]) -> Vec<u8> {
    [&[selector], body].concat()
}

/// Appends a chunk with a length byte.
fn push_chunk(out: &mut Vec<u8>, chunk: &[u8]) {
    let len = u8::try_from(chunk.len()).expect("a short chunk");
    out.push(len);
    out.extend_from_slice(chunk);
}

/// Appends a chunk with a two-byte length.
fn push_long_chunk(out: &mut Vec<u8>, chunk: &[u8]) {
    let len = u16::try_from(chunk.len()).expect("a chunk below 64 KiB");
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(chunk);
}

fn gateway_payload() -> Vec<Vec<u8>> {
    vec![
        with_selector(0, GATEWAY_SESSION.as_bytes()),
        with_selector(0, MESSAGE.as_bytes()),
    ]
}

fn gateway_compression() -> Vec<Vec<u8>> {
    let messages: Vec<String> = GATEWAY_SESSION.lines().map(str::to_owned).collect();
    let mut seeds = Vec::new();
    for (selector, frames) in [(0, zlib_frames(&messages)), (1, zstd_frames(&messages))] {
        let mut seed = vec![selector];
        for frame in frames.iter().filter(|frame| frame.len() < 256) {
            push_chunk(&mut seed, frame);
        }
        seeds.push(seed);
    }
    for selector in [2, 3] {
        let mut seed = vec![selector];
        for message in &messages {
            push_long_chunk(&mut seed, message.as_bytes());
        }
        seeds.push(seed);
    }
    seeds
}

fn http_response() -> Vec<Vec<u8>> {
    // Selectors: 0 is 200, 3 is 400, 8 is 429.
    vec![
        with_selector(0, format!("5\n4\n1.5\nabcd\n\n\n\n{MESSAGE}").as_bytes()),
        with_selector(3, format!("\n\n\n\n\n\n\n{ERROR_BODY}").as_bytes()),
        with_selector(
            8,
            format!("5\n0\n1e300\nabcd\ntrue\nglobal\n\n{RATE_LIMIT_BODY}").as_bytes(),
        ),
        with_selector(
            8,
            format!("1\n0\n2\nabcd\n\nuser\n3\n{RATE_LIMIT_BODY}").as_bytes(),
        ),
        // A wait time near the largest `Duration`: it once made `Instant + Duration` panic.
        with_selector(
            8,
            b"1\n0\n18446744073709550000\nabcd\ntrue\n\n18446744073709550000\n{}",
        ),
    ]
}

fn voice_packet() -> Vec<Vec<u8>> {
    let cipher = TransportCipher::new(EncryptionMode::Aes256GcmRtpSize, &SecretKey::new(KEY));
    let header = RtpHeader::new(5, 960, 1).encode();
    let sealed = cipher
        .encrypt_with_nonce(&header, &[0xFC, 1, 2, 3], 0)
        .expect("encryption");
    let discovery = IpDiscovery::new(1, "203.0.113.5".to_owned(), 50_000)
        .encode()
        .expect("a short address");

    let mut steps = sealed.clone();
    for (index, kind) in [1_u8, 1, 3, 0, 2, 3].into_iter().enumerate() {
        steps.push(kind);
        let mut chunk = header.to_vec();
        chunk[3] = u8::try_from(index).expect("small");
        chunk.extend_from_slice(&[0xFC, 9, 9, 9]);
        push_long_chunk(&mut steps, &chunk);
    }
    vec![
        with_selector(0, &steps),
        with_selector(0, &sealed),
        with_selector(1, &discovery),
    ]
}

fn voice_gateway() -> Vec<Vec<u8>> {
    let key: Vec<String> = (0..32).map(|byte: u8| byte.to_string()).collect();
    let session_description = format!(
        r#"{{"op":4,"d":{{"mode":"aead_aes256_gcm_rtpsize","secret_key":[{}],"dave_protocol_version":0}}}}"#,
        key.join(",")
    );
    let steps: [(u8, &[u8]); 9] = [
        (0, br#"{"op":8,"d":{"heartbeat_interval":13750.0}}"#),
        (
            0,
            br#"{"op":2,"d":{"ssrc":7,"ip":"127.0.0.1","port":50000,"modes":["aead_aes256_gcm_rtpsize","aead_xchacha20_poly1305_rtpsize"]}}"#,
        ),
        (4, b"203.0.113.5"),
        (0, session_description.as_bytes()),
        (0, br#"{"op":6,"d":{"t":1}}"#),
        (0, br#"{"op":11,"d":{"user_ids":["5"]}}"#),
        (0, br#"{"op":5,"d":{"user_id":"5","ssrc":9,"speaking":1}}"#),
        (2, &[0, 1, 25, 0, 1, 2]),
        (3, b""),
    ];
    let mut seed = Vec::new();
    for (kind, chunk) in steps {
        seed.push(kind);
        push_long_chunk(&mut seed, chunk);
    }
    vec![with_selector(0, &seed), with_selector(1, &seed)]
}

fn dave_frame() -> Vec<Vec<u8>> {
    let mut codec = FixedKeyCodec::new(&BASE_SECRET);
    let frame = codec.encrypt(&[0x78, 1, 2, 3, 4, 5]).expect("encryption");

    let mut steps = frame.clone();
    for flip in [0_u8, 3, 0, 200] {
        steps.push(flip);
        push_chunk(&mut steps, &[0xFC, flip, 1, 2]);
    }

    let mut session = Vec::new();
    for (op, payload) in [
        (
            4_u8,
            r#"{"transition_id":1,"protocol_version":1}"#.as_bytes(),
        ),
        (12, br#"{"protocol_version":1,"epoch":1}"#.as_slice()),
        (5, &frame),
        (6, &[0x78, 1, 2]),
        (0, &[0, 1, 2, 3]),
        (7, &[]),
    ] {
        session.push(op);
        push_long_chunk(&mut session, payload);
    }
    vec![with_selector(0, &steps), with_selector(3, &session)]
}

fn ogg() -> Vec<Vec<u8>> {
    let mut writer = OggOpusWriter::new(Vec::new(), 2, 7).expect("an Ogg writer");
    for packet in [[0xFC, 1, 2], [0xFC, 3, 4], [0xF8, 0xFF, 0xFE]] {
        writer.write_packet(&packet).expect("an Ogg packet");
    }
    let file = writer.finish().expect("an Ogg file");

    let mut pages = vec![1];
    pages.extend_from_slice(&[1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    push_chunk(&mut pages, b"tags");
    pages.push(0);
    push_chunk(&mut pages, &[3, 255, 1]);
    pages.extend_from_slice(&[0xFC; 259]);
    pages.push(4);
    push_chunk(&mut pages, &[2]);
    pages.extend_from_slice(&[0xFC, 0]);
    vec![with_selector(0, &file), pages]
}

fn endpoint() -> Vec<Vec<u8>> {
    vec![
        with_selector(0, PING.as_bytes()),
        with_selector(0, WEBHOOK_EVENT.as_bytes()),
        with_selector(1, PING.as_bytes()),
    ]
}

fn command_parse() -> Vec<Vec<u8>> {
    vec![
        with_selector(0, COMMAND_DATA.as_bytes()),
        with_selector(1, b"vote:{poll}:{choice}\nvote:12:yes"),
        with_selector(1, b"{a}-{b}-x\n1-2-3-x"),
        with_selector(1, "page:{n}\u{e9}\npage:\u{e9}\u{e9}".as_bytes()),
    ]
}
