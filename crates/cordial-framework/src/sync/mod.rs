//! Command registration: compare the local command definitions with the commands on Discord
//! and send only the changes.
//!
//! See `docs/ARCHITECTURE.md`, section 9.3.
//!
//! ```no_run
//! # async fn run(client: cordial_http::Client) -> Result<(), cordial_framework::sync::SyncError> {
//! use cordial_framework::sync::{SyncCommand, SyncOptions, SyncScope, sync};
//! use cordial_model::command::CommandDefinition;
//! use cordial_model::id::ApplicationId;
//!
//! let commands = [
//!     SyncCommand::new(CommandDefinition::chat_input("ping", "Check the bot")),
//!     SyncCommand::new(CommandDefinition::chat_input("ban", "Ban a member")).previous_name("kick"),
//! ];
//! let report = sync(
//!     &client,
//!     ApplicationId::new_const(1234),
//!     SyncScope::Global,
//!     &commands,
//!     &SyncOptions::new(),
//! )
//! .await?;
//! // For example "created 1, updated 0, renamed 1, deleted 0, unchanged 0".
//! let summary = report.plan.to_string();
//! # Ok(())
//! # }
//! ```

mod apply;
mod normalize;
mod plan;

use std::fmt;

use cordial_model::command::CommandDefinition;
use cordial_model::id::GuildId;

pub use apply::{SyncError, SyncOutcome, SyncReport, sync};
pub use plan::{Action, Plan, PlanError, plan};

/// A local command definition for the sync, with the names that the command had before.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct SyncCommand {
    /// The definition to register.
    pub definition: CommandDefinition,
    /// Earlier names of the command. A remote command with one of these names (and the same
    /// type) gets the new name. It keeps its ID and its permission overwrites.
    pub previous_names: Vec<String>,
}

impl SyncCommand {
    /// Makes a sync command with no earlier names.
    #[must_use]
    pub fn new(definition: CommandDefinition) -> Self {
        Self {
            definition,
            previous_names: Vec::new(),
        }
    }

    /// Adds an earlier name of the command.
    #[must_use]
    pub fn previous_name(mut self, name: impl Into<String>) -> Self {
        self.previous_names.push(name.into());
        self
    }
}

impl From<CommandDefinition> for SyncCommand {
    fn from(definition: CommandDefinition) -> Self {
        Self::new(definition)
    }
}

/// The set of commands to sync: the global commands or the commands of one guild.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SyncScope {
    /// The global commands of the application.
    Global,
    /// The commands of the application in one guild.
    Guild(GuildId),
}

impl fmt::Display for SyncScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Global => formatter.write_str("global"),
            Self::Guild(guild_id) => write!(formatter, "guild {guild_id}"),
        }
    }
}

/// How the sync sends the changes of a plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SyncStrategy {
    /// One request for each change. Unchanged commands stay untouched.
    #[default]
    Granular,
    /// One bulk overwrite (`PUT`) with all commands, when the plan has changes.
    Bulk,
    /// `Bulk` when the plan has more changes than the threshold, else `Granular`.
    Auto {
        /// The largest number of changes that the sync sends one by one.
        threshold: usize,
    },
}

impl SyncStrategy {
    /// Tells if the sync sends one bulk overwrite for a plan with this number of changes.
    #[must_use]
    pub fn uses_bulk(self, change_count: usize) -> bool {
        match self {
            Self::Granular => false,
            Self::Bulk => true,
            Self::Auto { threshold } => change_count > threshold,
        }
    }
}

/// The settings of a sync.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SyncOptions {
    /// How the sync sends the changes.
    pub strategy: SyncStrategy,
    /// When `true`, the sync makes the plan and sends no change.
    pub dry_run: bool,
    /// When `true`, the sync deletes an Entry Point command that has no local definition.
    pub delete_entry_point: bool,
}

impl SyncOptions {
    /// Makes the default settings: granular strategy, no dry run, keep the Entry Point command.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the strategy.
    #[must_use]
    pub fn strategy(mut self, strategy: SyncStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Sets the dry-run mode.
    #[must_use]
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Sets whether the sync deletes an Entry Point command that has no local definition.
    #[must_use]
    pub fn delete_entry_point(mut self, delete_entry_point: bool) -> Self {
        self.delete_entry_point = delete_entry_point;
        self
    }
}
