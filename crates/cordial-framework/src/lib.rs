//! Event router, command system, and component router for Discord bots.
//!
//! The framework connects the other layers: the HTTP client, the shards, and the cache.
//! A [`Bot`] runs typed handlers for commands, components, modals, and gateway events.
//! See `docs/ARCHITECTURE.md`, section 9, and the guide book in `docs/guide`.
//!
//! | Part | Items |
//! |---|---|
//! | The bot and its settings | [`Bot`], [`BotBuilder`], [`Context`] |
//! | Commands | [`Command`] (derive and trait), [`CommandGroup`], [`Choice`], [`command`] |
//! | Responses | [`InteractionContext`], [`ResponseState`], [`SlowResponse`] |
//! | Components and modals | [`ComponentContext`], [`Pattern`], [`Modal`] |
//! | Checks | [`Check`], [`Cooldown`] |
//! | Command registration | [`sync`] |
//!
//! ```no_run
//! use cordial_framework::{Bot, BoxError, Command, CommandContext};
//! use cordial_http::Token;
//!
//! /// Checks that the bot is online.
//! #[derive(Command)]
//! #[command(name = "ping", description = "Check that the bot is online")]
//! # // A doctest of this crate needs the path: the macro sees the crate itself.
//! # #[command(crate = "cordial_framework")]
//! struct Ping;
//!
//! async fn ping(ctx: CommandContext<()>, _: Ping) -> Result<(), BoxError> {
//!     ctx.reply("Pong!").await?;
//!     Ok(())
//! }
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let bot = Bot::builder(Token::from_env("DISCORD_TOKEN")?)
//!     .command(ping)
//!     .build()?;
//! bot.run().await?;
//! # Ok(())
//! # }
//! ```

mod bot;
pub mod check;
pub mod collector;
pub mod command;
pub mod component;
mod context;
#[cfg(feature = "endpoint")]
pub mod endpoint;
pub mod error;
pub mod intents;
pub mod interaction;
pub mod localize;
pub mod modal;
mod router;
pub mod sync;

pub use bot::{
    Bot, BotBuilder, CommandSettings, CommandSync, EventOrder, RunOutcome, ShutdownMode,
};
pub use check::{Check, CheckFailure, Cooldown, CooldownScope};
pub use command::{Command, CommandArg, CommandOptions, CommandTarget, ParseError};
pub use component::{ComponentContext, Params, Pattern};
pub use context::Context;
pub use cordial_macros::{Choice, Command, CommandGroup, Modal};
pub use error::{BoxError, Error, HandlerError, HandlerResult};
pub use intents::IntentCheck;
pub use interaction::{CommandContext, InteractionContext, ResponseState, SlowResponse};
pub use localize::Localizer;
pub use modal::Modal;
pub use router::{CommandScope, Focused};

/// Items for the code that the derive macros generate. Not a public API.
#[doc(hidden)]
pub mod __private {
    pub use cordial_model as model;
}

#[cfg(test)]
mod tests;
