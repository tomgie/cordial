//! Discord gateway shards and the shard manager.
//!
//! The protocol logic is a sans-IO core ([`core::ShardCore`]). A Tokio driver
//! ([`Shard`]) owns the socket and the timers. See `docs/ARCHITECTURE.md`, section 7.
//!
//! ```no_run
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use cordial_gateway::{Shard, ShardConfig, ShardEvent};
//! use cordial_http::Token;
//! use cordial_model::gateway::{Event, Intents};
//!
//! let config = ShardConfig::new(Token::bot("TOKEN"), Intents::GUILDS | Intents::GUILD_MESSAGES);
//! let (_shard, mut events) = Shard::start(config);
//! while let Some(message) = events.recv().await {
//!     if let ShardEvent::Dispatch(event) = message.event {
//!         if let Event::MessageCreate(created) = *event {
//!             println!("{}", created.content);
//!         }
//!     }
//! }
//! # Ok(())
//! # }
//! ```

pub mod compression;
pub mod core;
mod manager;
pub mod queue;
mod shard;

pub use compression::Compression;
pub use core::{Command, CommandError, FatalError, Lifecycle, Session};
pub use manager::{
    ManagerError, SESSION_START_RESERVE, ShardManager, ShardManagerBuilder, ShardRange,
    shard_id_for_guild,
};
pub use queue::{IdentifyQueue, LocalIdentifyQueue};
pub use shard::{
    CloseMode, DEFAULT_GATEWAY_URL, Shard, ShardConfig, ShardError, ShardEvent, ShardHandle,
    ShardMessage,
};
