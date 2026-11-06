//! The inflate rate of `zlib-stream` for a realistic stream of gateway events.

use std::hint::black_box;

use cordial_gateway::Compression;
use cordial_gateway::compression::Inflater;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use serde_json::{Value, json};

#[path = "../../cordial-model/benches/support/events.rs"]
mod events;

/// The number of events in the stream.
const EVENT_COUNT: u64 = 500;

fn dispatch(sequence: u64, name: &str, data: &Value) -> String {
    json!({"op": 0, "s": sequence, "t": name, "d": data}).to_string()
}

/// Gives the messages of a session: Hello, one large GUILD_CREATE, then mostly messages and
/// presences, with a heartbeat acknowledgement now and then.
fn session_messages() -> Vec<String> {
    let mut messages = vec![
        json!({"op": 10, "d": {"heartbeat_interval": 41_250}}).to_string(),
        dispatch(
            1,
            "GUILD_CREATE",
            &events::guild_create(events::GUILD_ID, events::LARGE_GUILD),
        ),
    ];
    for sequence in 2..EVENT_COUNT {
        let message = match sequence % 10 {
            0 => json!({"op": 11}).to_string(),
            1..=3 => dispatch(sequence, "PRESENCE_UPDATE", &events::presence_update()),
            4 => dispatch(
                sequence,
                "INTERACTION_CREATE",
                &events::interaction_create(),
            ),
            _ => dispatch(
                sequence,
                "MESSAGE_CREATE",
                &events::message_create(sequence),
            ),
        };
        messages.push(message);
    }
    messages
}

/// Compresses messages like the gateway: one zlib stream, a sync flush after each message.
fn compress(messages: &[String]) -> Vec<Vec<u8>> {
    let mut compressor = flate2::Compress::new(flate2::Compression::default(), true);
    messages
        .iter()
        .map(|message| {
            let mut frame = Vec::with_capacity(message.len() + 1024);
            compressor
                .compress_vec(message.as_bytes(), &mut frame, flate2::FlushCompress::Sync)
                .expect("compression");
            frame
        })
        .collect()
}

fn inflate_stream(criterion: &mut Criterion) {
    let messages = session_messages();
    let frames = compress(&messages);
    let inflated_size: usize = messages.iter().map(String::len).sum();

    let mut group = criterion.benchmark_group("gateway_inflate");
    group.throughput(Throughput::Bytes(
        u64::try_from(inflated_size).expect("a small stream"),
    ));
    group.bench_function("zlib_stream_session", |bencher| {
        bencher.iter(|| {
            let mut inflater = Inflater::new(Compression::Zlib).expect("zlib is compiled in");
            for frame in &frames {
                black_box(inflater.push(black_box(frame)).expect("a valid frame"));
            }
        });
    });
    group.finish();
}

criterion_group!(benches, inflate_stream);
criterion_main!(benches);
