//! The heap memory of the cache: bytes for each guild, member, channel, and role.
//!
//! A counting global allocator measures the net heap growth of the cache while it takes
//! GUILD_CREATE events. The events are decoded before the measurement, so only the cache counts.
//! Run it with `cargo bench -p cordial-cache --bench cache_memory`.

#![allow(
    clippy::print_stdout,
    reason = "the benchmark reports its numbers on stdout"
)]

use std::alloc::System;

use cordial_cache::{Cache, ResourceTypes};
use cordial_model::gateway::event::Event;
use serde_json::value::RawValue;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};

#[path = "../../cordial-model/benches/support/events.rs"]
mod events;

use events::GuildSize;

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

/// The number of items for each measurement. Large counts make the fixed costs small.
const GUILDS: u64 = 1_000;
const MEMBERS: u64 = 10_000;
const CHANNELS: u64 = 2_000;
const ROLES: u64 = 1_000;

const EMPTY_GUILD: GuildSize = GuildSize {
    members: 0,
    channels: 0,
    roles: 1,
};

fn guild_event(guild_id: u64, size: GuildSize) -> Event {
    let data = events::guild_create(guild_id, size).to_string();
    let raw = RawValue::from_string(data).expect("valid JSON");
    Event::decode("GUILD_CREATE", &raw).expect("the generated event decodes")
}

/// Gives the net heap growth in bytes of a new cache after it takes the events.
fn heap_growth(events: &[Event]) -> i64 {
    let region = Region::new(GLOBAL);
    let cache = Cache::new(ResourceTypes::all());
    for event in events {
        cache.update(event);
    }
    let change = region.change();
    drop(cache);

    let allocated = i64::try_from(change.bytes_allocated).expect("a small heap");
    let deallocated = i64::try_from(change.bytes_deallocated).expect("a small heap");
    let reallocated = i64::try_from(change.bytes_reallocated).expect("a small heap");
    allocated - deallocated + reallocated
}

/// Gives the heap bytes for each item: the growth of a guild with `count` items, less the
/// growth of the same guild without them.
fn bytes_per_item(count: u64, with_items: GuildSize, without_items: GuildSize) -> i64 {
    let full = heap_growth(&[guild_event(events::GUILD_ID, with_items)]);
    let base = heap_growth(&[guild_event(events::GUILD_ID, without_items)]);
    (full - base) / i64::try_from(count).expect("a small count")
}

fn main() {
    let guilds: Vec<Event> = (0..GUILDS)
        .map(|index| guild_event(events::GUILD_ID + index * 1_000_000, EMPTY_GUILD))
        .collect();
    let per_guild = heap_growth(&guilds) / i64::try_from(GUILDS).expect("a small count");

    let per_member = bytes_per_item(
        MEMBERS,
        GuildSize {
            members: MEMBERS,
            channels: 0,
            roles: 2,
        },
        GuildSize {
            members: 0,
            channels: 0,
            roles: 2,
        },
    );
    let per_channel = bytes_per_item(
        CHANNELS,
        GuildSize {
            channels: CHANNELS,
            ..EMPTY_GUILD
        },
        EMPTY_GUILD,
    );
    let per_role = bytes_per_item(
        ROLES,
        GuildSize {
            roles: ROLES + 1,
            ..EMPTY_GUILD
        },
        EMPTY_GUILD,
    );
    let large_guild = heap_growth(&[guild_event(events::GUILD_ID, events::LARGE_GUILD)]);

    println!("cache heap memory (net bytes, ResourceTypes::all())");
    println!("| Item | Bytes |");
    println!("|---|---:|");
    println!("| Guild (with the @everyone role, no members or channels) | {per_guild} |");
    println!("| Member (with its shared user) | {per_member} |");
    println!("| Channel (with 2 permission overwrites) | {per_channel} |");
    println!("| Role | {per_role} |");
    println!("| Large guild (1,000 members, 200 channels, 100 roles) | {large_guild} |");
}
