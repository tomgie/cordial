# Benchmarks

The benchmarks use `criterion`. Each benchmark is in the crate that it measures (`benches/*.rs`). CI does not run them.

The numbers below are indicative only. They come from one short run on one developer machine. Use them to compare changes on the same machine, not as a promise.

## Machine

| Item | Value |
|---|---|
| CPU | AMD Ryzen 9 5900X (12 cores, 24 threads) |
| Memory | 32 GB |
| OS | Windows 11 Pro (10.0.26200), x86_64 MSVC |
| Compiler | rustc 1.99.1, `bench` profile |
| Settings | 1 s warm-up, 2 s measurement (`-- --warm-up-time 1 --measurement-time 2`) |
| Date | 2026-11-05 |

## Test data

`crates/cordial-model/benches/support/events.rs` generates the event data. The model, gateway, and cache benchmarks share it.

| Event | Content | Size of `d` |
|---|---|---:|
| MESSAGE_CREATE | Guild message with author, member, one mention, and one embed with 2 fields | 1.1 KB |
| GUILD_CREATE | Large guild: 1,000 members, 200 channels (10 categories), 100 roles | 450 KB |
| PRESENCE_UPDATE | One game activity with timestamps and assets | 0.4 KB |
| INTERACTION_CREATE | Slash command with 3 options, a resolved user and member | 1.3 KB |

## Results

Times are the middle value of the criterion estimate.

| Benchmark | Command | Time | Rate |
|---|---|---:|---:|
| `Event::decode` MESSAGE_CREATE | `cargo bench -p cordial-model` | 7.9 µs | 132 MiB/s, 127,000 events/s |
| `Event::decode` GUILD_CREATE (large) | `cargo bench -p cordial-model` | 3.6 ms | 119 MiB/s, 280 events/s |
| `Event::decode` PRESENCE_UPDATE | `cargo bench -p cordial-model` | 2.2 µs | 193 MiB/s, 454,000 events/s |
| `Event::decode` INTERACTION_CREATE | `cargo bench -p cordial-model` | 8.3 µs | 153 MiB/s, 120,000 events/s |
| `zlib-stream` inflate, session of 500 events (877 KB) | `cargo bench -p cordial-gateway` | 443 µs | 1.85 GiB/s |
| `Cache::update` GUILD_CREATE (large, empty cache) | `cargo bench -p cordial-cache --bench cache_update` | 932 µs | 1,070 guilds/s |
| `Cache::update` MESSAGE_CREATE (ring buffer of 100) | `cargo bench -p cordial-cache --bench cache_update` | 1.8 µs | 556,000 events/s |
| `InMemoryRateLimiter` acquire and complete | `cargo bench -p cordial-http` | 486 ns | |
| `NoRateLimiter` acquire and complete (baseline) | `cargo bench -p cordial-http` | 164 ns | |
| `RateLimitHeaders::parse` (4 headers) | `cargo bench -p cordial-http` | 432 ns | |
| Command option parsing (derived, 4 options, resolved user) | `cargo bench -p cordial-framework` | 275 ns | |
| `Pattern::matches`, match with 2 parameters | `cargo bench -p cordial-framework` | 425 ns | |
| `Pattern::matches`, no match | `cargo bench -p cordial-framework` | 13 ns | |

The rate limiter overhead for each request is the difference to the baseline: about 320 ns. Both numbers include one `block_on` on a current-thread Tokio runtime. The limiter has a global limit of `u32::MAX` in the benchmark, so that no request waits.

## Cache memory

`cargo bench -p cordial-cache --bench cache_memory` measures the heap with a counting global allocator (`stats_alloc`). It counts the net heap growth of a new cache (`ResourceTypes::all()`) while it takes GUILD_CREATE events. The events are decoded before the measurement. Each value per item is the growth of a guild with many items, less the growth of the same guild without them, divided by the item count. Thus the values include the amortized growth of the maps.

| Item | Heap bytes |
|---|---:|
| Guild (with the @everyone role, no members or channels) | 1,727 |
| Member (with its shared user) | 981 |
| Channel (with 2 permission overwrites) | 1,350 |
| Role | 238 |
| Large guild (1,000 members, 200 channels, 100 roles) | 1,658,695 |

The stack sizes of the cached types are in the `cached_type_sizes` test of `cordial-cache`.

## Run the benchmarks

```sh
cargo bench -p cordial-model
cargo bench -p cordial-gateway
cargo bench -p cordial-cache --bench cache_update
cargo bench -p cordial-cache --bench cache_memory
cargo bench -p cordial-http --bench rate_limiter
cargo bench -p cordial-framework --bench command
```

Add `-- --warm-up-time 1 --measurement-time 2` for a short run. Give a `--bench` name when you add criterion options: the library bench target of a crate does not accept them.
