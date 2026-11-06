//! The decode rate of gateway events: `Event::decode` from the raw `d` value.

use std::hint::black_box;

use cordial_model::gateway::event::Event;
use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use serde_json::value::RawValue;

#[path = "support/events.rs"]
mod events;

fn raw(value: &serde_json::Value) -> Box<RawValue> {
    RawValue::from_string(value.to_string()).expect("valid JSON")
}

fn decode_events(criterion: &mut Criterion) {
    let cases = [
        ("MESSAGE_CREATE", raw(&events::message_create(1))),
        (
            "GUILD_CREATE",
            raw(&events::guild_create(events::GUILD_ID, events::LARGE_GUILD)),
        ),
        ("PRESENCE_UPDATE", raw(&events::presence_update())),
        ("INTERACTION_CREATE", raw(&events::interaction_create())),
    ];

    let mut group = criterion.benchmark_group("event_decode");
    for (name, data) in &cases {
        // Check once that the data is valid, so that the benchmark measures a real decode.
        let event = Event::decode(name, data).expect("the generated event decodes");
        assert_eq!(event.name(), *name);

        let size = u64::try_from(data.get().len()).expect("a small event");
        group.throughput(Throughput::Bytes(size));
        group.bench_function(*name, |bencher| {
            bencher.iter(|| Event::decode(black_box(name), black_box(data)));
        });
    }
    group.finish();
}

criterion_group!(benches, decode_events);
criterion_main!(benches);
