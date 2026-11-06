//! Discord gateway shards and the shard manager.
//!
//! The protocol logic is a sans-IO core. A Tokio driver owns the socket and the timers.
//! See `docs/ARCHITECTURE.md`, section 7.
