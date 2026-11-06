//! Event router, command system, and component router for Discord bots.
//!
//! See `docs/ARCHITECTURE.md`, section 9.

pub mod command;
pub mod modal;

pub use command::{Command, CommandArg, CommandOptions, CommandTarget, ParseError};
pub use cordial_macros::{Choice, Command, CommandGroup, Modal};
pub use modal::Modal;

/// Items for the code that the derive macros generate. Not a public API.
#[doc(hidden)]
pub mod __private {
    pub use cordial_model as model;
}
