//! Data types for the Discord API.
//!
//! This crate has no I/O. It contains IDs, objects, enums, bit flags, gateway events,
//! the permission calculator, builders, and client-side validation.
//! See `docs/ARCHITECTURE.md`, section 5.
//!
//! # Forward compatibility
//!
//! Discord adds fields, enum values, and flag bits without a new API version.
//! Every enum has an `Unknown` variant, and every flag type keeps unknown bits.
//! Structs are `#[non_exhaustive]`.
//!
//! # Example
//!
//! IDs are typed: a [`id::UserId`] cannot go where a [`id::ChannelId`] is necessary.
//! Builders make the objects that requests take.
//!
//! ```
//! use cordial_model::embed::Embed;
//! use cordial_model::id::UserId;
//! use cordial_model::interaction::InteractionMessage;
//! use cordial_model::{Color, Permissions};
//!
//! let user_id: UserId = "80351110224678912".parse().unwrap();
//! assert_eq!(user_id.get(), 80_351_110_224_678_912);
//!
//! let embed = Embed::new()
//!     .title("Status")
//!     .description("All systems work.")
//!     .color(Color::from_rgb(0x2E, 0xCC, 0x71));
//! let message = InteractionMessage::text("Report").embed(embed).ephemeral();
//! assert_eq!(message.content.as_deref(), Some("Report"));
//!
//! let permissions = Permissions::SEND_MESSAGES | Permissions::VIEW_CHANNEL;
//! assert!(permissions.contains(Permissions::VIEW_CHANNEL));
//! ```

#[macro_use]
mod macros;

pub mod application;
pub mod audit_log;
pub mod auto_moderation;
pub mod cdn;
pub mod channel;
pub mod color;
pub mod command;
pub mod component;
pub mod embed;
pub mod emoji;
pub mod gateway;
pub mod guild;
pub mod id;
pub mod image;
pub mod interaction;
pub mod invite;
pub mod lobby;
pub mod locale;
pub mod mention;
pub mod message;
pub mod monetization;
pub mod nullable;
pub mod oauth2;
pub mod permissions;
pub mod poll;
pub mod scheduled_event;
pub mod secret;
pub mod serde_util;
pub mod soundboard;
pub mod sticker;
pub mod time;
pub mod user;
pub mod validate;
pub mod voice;
pub mod webhook;
pub mod webhook_event;

pub use color::Color;
pub use id::Id;
pub use image::ImageHash;
pub use locale::Locale;
pub use nullable::{Nullable, Patch};
pub use permissions::Permissions;
pub use secret::Secret;
pub use time::Timestamp;

/// The version of the Discord API that this library uses.
pub const API_VERSION: u8 = 10;
