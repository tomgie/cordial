//! A full-featured library for Discord bots.
//!
//! This crate re-exports the `cordial-*` crates. Each crate is one layer.
//! Turn on only the layers that your bot needs with Cargo features.
//!
//! | Feature | Crate | Default |
//! |---|---|---|
//! | (always) | [`model`] | yes |
//! | `http` | `http` | yes |
//! | `gateway` | `gateway` | yes |
//! | `cache` | `cache` | yes |
//! | `framework` | `framework` | yes |
//! | `macros` | `macros` | yes |
//! | `endpoint` | `endpoint` | no |
//! | `voice` | `voice` | no |

pub use cordial_model as model;

#[cfg(feature = "http")]
pub use cordial_http as http;

#[cfg(feature = "gateway")]
pub use cordial_gateway as gateway;

#[cfg(feature = "cache")]
pub use cordial_cache as cache;

#[cfg(feature = "endpoint")]
pub use cordial_endpoint as endpoint;

#[cfg(feature = "voice")]
pub use cordial_voice as voice;

#[cfg(feature = "macros")]
pub use cordial_macros as macros;

#[cfg(feature = "framework")]
pub use cordial_framework as framework;
