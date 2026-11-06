//! Discord gateway shards and the shard manager.
//!
//! The protocol logic is a sans-IO core ([`core::ShardCore`]). A Tokio driver
//! ([`Shard`]) owns the socket and the timers. See `docs/ARCHITECTURE.md`, section 7.

pub mod core;
