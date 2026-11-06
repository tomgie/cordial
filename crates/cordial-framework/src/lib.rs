//! Event router, command system, and component router for Discord bots.
//!
//! See `docs/ARCHITECTURE.md`, section 9.

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
