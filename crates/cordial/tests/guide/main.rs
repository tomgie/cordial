//! The code samples of the guide book in `docs/guide`.
//!
//! The guide includes the code between the `ANCHOR` comments. This test crate makes sure
//! that the samples compile against the current API.

// The samples are functions that the guide shows. Nothing calls them.
#![allow(dead_code)]

mod cache;
mod commands;
mod components;
mod deploy;
mod events;
mod http;
mod intents;
mod responses;
mod sharding;
#[cfg(feature = "voice")]
mod voice;
