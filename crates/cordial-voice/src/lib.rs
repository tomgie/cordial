//! Discord voice connections.
//!
//! This crate contains the voice gateway, UDP transport, transport encryption,
//! DAVE end-to-end encryption, and the audio pipeline.
//! See `docs/ARCHITECTURE.md`, section 11.

pub mod dave;
pub mod e2ee;
