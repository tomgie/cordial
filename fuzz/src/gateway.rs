//! The gateway: frame decode, event decode, the shard core, the cache, and decompression.

use std::time::{Duration, Instant};

use cordial_cache::{Cache, ResourceTypes};
use cordial_gateway::Compression;
use cordial_gateway::compression::Inflater;
use cordial_gateway::core::{Action, CoreConfig, ShardCore};
use cordial_model::gateway::Intents;
use cordial_model::gateway::event::{Event, EventType};
use serde_json::value::RawValue;

use crate::reader::Reader;

/// Gateway text frames.
///
/// The selector byte chooses an event name from [`EventType::ALL`] for a direct decode of the
/// rest. Then each line of the rest goes to a shard core as one frame. The dispatch events of
/// the core go to a cache.
pub fn gateway_payload(data: &[u8]) {
    let mut reader = Reader::new(data);
    let selector = reader.byte();
    let Ok(text) = std::str::from_utf8(reader.rest()) else {
        return;
    };

    decode_named_event(selector, text);
    run_shard_core(text);
}

fn decode_named_event(selector: u8, text: &str) {
    let kinds = EventType::ALL;
    let name = kinds[usize::from(selector) % kinds.len()].name();
    let Ok(raw) = serde_json::from_str::<&RawValue>(text) else {
        return;
    };

    if let Ok(event) = Event::decode(name, raw) {
        assert_eq!(event.name(), name);
        let cache = Cache::new(ResourceTypes::all());
        cache.update(&event);
    }
}

fn run_shard_core(text: &str) {
    let start = Instant::now();
    let mut core = ShardCore::new(CoreConfig::new("token", Intents::all()), start);
    let cache = Cache::new(ResourceTypes::all());
    core.connected(start);

    for (index, frame) in text.split('\n').enumerate() {
        let now = start + Duration::from_secs(u64::try_from(index).unwrap_or(0));
        let _ = core.received(frame, now);
        core.tick(now);
        drain_actions(&mut core, &cache, now);
    }
    let _ = core.session();
    let _ = core.next_deadline();
}

fn drain_actions(core: &mut ShardCore, cache: &Cache, now: Instant) {
    while let Some(action) = core.poll_action() {
        match action {
            Action::RequestIdentify => core.identify(now),
            Action::Dispatch { name, data } => {
                if let Ok(event) = Event::decode(&name, &data) {
                    cache.update(&event);
                }
            }
            Action::Reconnect { .. } => core.connected(now),
            _ => {}
        }
    }
}

/// Transport decompression.
///
/// Selector bit 0 chooses `zlib-stream` or `zstd-stream`. With bit 1 clear, the rest is a
/// list of chunks (a length byte, then the bytes) that go to the decompressor as frames. With
/// bit 1 set, the chunks are messages: the body compresses them like the gateway, and checks
/// that the decompressor gives them back.
pub fn gateway_compression(data: &[u8]) {
    let mut reader = Reader::new(data);
    let selector = reader.byte();
    let compression = if selector & 1 == 0 {
        Compression::Zlib
    } else {
        Compression::Zstd
    };

    if selector & 2 == 0 {
        inflate_chunks(compression, reader);
    } else {
        round_trip(compression, reader);
    }
}

fn inflate_chunks(compression: Compression, mut reader: Reader<'_>) {
    let mut inflater = Inflater::new(compression).expect("the compression is compiled in");
    while !reader.is_empty() {
        let _ = inflater.push(reader.chunk());
    }
}

fn round_trip(compression: Compression, mut reader: Reader<'_>) {
    let mut messages = Vec::new();
    while !reader.is_empty() {
        let message = String::from_utf8_lossy(reader.long_chunk()).into_owned();
        // The gateway never sends an empty message, and its compressed frame is empty.
        if !message.is_empty() {
            messages.push(message);
        }
    }
    let frames = match compression {
        Compression::Zlib => zlib_frames(&messages),
        _ => zstd_frames(&messages),
    };

    let mut inflater = Inflater::new(compression).expect("the compression is compiled in");
    for (frame, message) in frames.iter().zip(&messages) {
        let text = inflater.push(frame).expect("a valid frame");
        assert_eq!(text.as_deref(), Some(message.as_str()));
    }
}

/// Compresses messages like the gateway: one zlib stream, a sync flush after each message.
pub fn zlib_frames(messages: &[String]) -> Vec<Vec<u8>> {
    let mut compressor = flate2::Compress::new(flate2::Compression::default(), true);
    messages
        .iter()
        .map(|message| {
            let mut output = Vec::with_capacity(message.len() * 2 + 64);
            compressor
                .compress_vec(message.as_bytes(), &mut output, flate2::FlushCompress::Sync)
                .expect("zlib compression");
            output
        })
        .collect()
}

/// Compresses messages like the gateway: one zstd stream, a flush after each message.
pub fn zstd_frames(messages: &[String]) -> Vec<Vec<u8>> {
    use zstd::stream::raw::{Encoder, InBuffer, Operation, OutBuffer};

    let mut encoder = Encoder::new(1).expect("zstd encoder");
    messages
        .iter()
        .map(|message| {
            let mut output = vec![0_u8; message.len() * 2 + 1024];
            let mut out = OutBuffer::around(&mut output[..]);
            let mut input = InBuffer::around(message.as_bytes());
            while input.pos() < message.len() {
                encoder.run(&mut input, &mut out).expect("zstd compression");
            }
            while encoder.flush(&mut out).expect("zstd flush") > 0 {}
            let written = out.pos();
            output.truncate(written);
            output
        })
        .collect()
}
