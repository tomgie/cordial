//! The update rate of the cache for GUILD_CREATE (a large guild) and MESSAGE_CREATE.

use std::hint::black_box;

use cordial_cache::{Cache, ResourceTypes};
use cordial_model::gateway::event::Event;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use serde_json::value::RawValue;

#[path = "../../cordial-model/benches/support/events.rs"]
mod events;

fn event(name: &str, data: &serde_json::Value) -> Event {
    let raw = RawValue::from_string(data.to_string()).expect("valid JSON");
    Event::decode(name, &raw).expect("the generated event decodes")
}

fn update(criterion: &mut Criterion) {
    let guild_create = event(
        "GUILD_CREATE",
        &events::guild_create(events::GUILD_ID, events::LARGE_GUILD),
    );
    let message_create = event("MESSAGE_CREATE", &events::message_create(1));

    let mut group = criterion.benchmark_group("cache_update");
    group.bench_function("GUILD_CREATE_large", |bencher| {
        // Each update fills an empty cache, so the guild is new each time.
        bencher.iter_batched(
            || Cache::new(ResourceTypes::all()),
            |cache| {
                cache.update(black_box(&guild_create));
                cache
            },
            BatchSize::SmallInput,
        );
    });

    let cache = Cache::with_message_capacity(ResourceTypes::all(), 100);
    cache.update(&guild_create);
    group.bench_function("MESSAGE_CREATE", |bencher| {
        bencher.iter(|| cache.update(black_box(&message_create)));
    });
    group.finish();
}

criterion_group!(benches, update);
criterion_main!(benches);
