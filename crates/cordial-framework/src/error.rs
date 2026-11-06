//! Errors of the framework and of handlers.

use std::fmt;

use cordial_model::gateway::Intents;

/// An error that a handler gives. Any error type converts into it with `?`.
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// The result of a handler.
pub type HandlerResult = Result<(), BoxError>;

/// A value that a handler can return: `()` or a `Result<(), E>`.
pub trait IntoHandlerResult {
    /// Converts the value.
    fn into_handler_result(self) -> HandlerResult;
}

impl IntoHandlerResult for () {
    fn into_handler_result(self) -> HandlerResult {
        Ok(())
    }
}

impl<E: Into<BoxError>> IntoHandlerResult for Result<(), E> {
    fn into_handler_result(self) -> HandlerResult {
        self.map_err(Into::into)
    }
}

/// An error that stops the bot.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// An HTTP request failed.
    #[error(transparent)]
    Http(#[from] cordial_http::Error),
    /// The shard manager did not start.
    #[error(transparent)]
    Gateway(#[from] cordial_gateway::ManagerError),
    /// The intent check failed in strict mode, or a privileged intent is not on.
    #[error("{0}")]
    Intents(IntentProblem),
    /// A shard stopped because of a fatal error.
    #[error("shard {shard_id} stopped: {message}")]
    ShardFatal {
        /// The shard ID.
        shard_id: u32,
        /// The message.
        message: String,
    },
    /// The command sync failed.
    #[error("the command sync failed: {0}")]
    Sync(BoxError),
    /// The settings of the bot are not valid.
    #[error("the bot settings are not valid: {0}")]
    Config(String),
}

/// A problem with the intents that stops the bot.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum IntentProblem {
    /// The bot requests privileged intents that are not on in the Developer Portal.
    PrivilegedNotEnabled(Intents),
    /// Handlers or the cache need intents that are not set (strict mode only).
    Missing(Intents),
}

impl fmt::Display for IntentProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PrivilegedNotEnabled(intents) => write!(
                formatter,
                "the privileged intents {intents:?} are not on for this application; turn them \
                 on in the Developer Portal (Bot page, Privileged Gateway Intents) or remove them"
            ),
            Self::Missing(intents) => write!(
                formatter,
                "the handlers or the cache need the intents {intents:?}, but they are not set"
            ),
        }
    }
}

/// Where a handler error came from.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorSource {
    /// An event handler, with the event name.
    Event(String),
    /// A command handler, with the command name.
    Command(String),
    /// An autocomplete handler, with the command name.
    Autocomplete(String),
    /// A component handler, with the custom ID.
    Component(String),
    /// A modal handler, with the custom ID.
    Modal(String),
}

impl fmt::Display for ErrorSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Event(name) => write!(formatter, "event handler for {name}"),
            Self::Command(name) => write!(formatter, "command `{name}`"),
            Self::Autocomplete(name) => write!(formatter, "autocomplete of `{name}`"),
            Self::Component(id) => write!(formatter, "component `{id}`"),
            Self::Modal(id) => write!(formatter, "modal `{id}`"),
        }
    }
}

/// An error of a handler, given to the error hook.
#[derive(Debug)]
#[non_exhaustive]
pub struct HandlerError {
    /// Where the error came from.
    pub source: ErrorSource,
    /// The error.
    pub kind: HandlerErrorKind,
}

/// The kind of a [`HandlerError`].
#[derive(Debug)]
#[non_exhaustive]
pub enum HandlerErrorKind {
    /// The handler gave an error.
    Error(BoxError),
    /// The handler panicked. The bot continues.
    Panic(String),
    /// The interaction data does not match the command definition.
    Parse(crate::command::ParseError),
}

impl fmt::Display for HandlerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            HandlerErrorKind::Error(error) => write!(formatter, "{} failed: {error}", self.source),
            HandlerErrorKind::Panic(message) => {
                write!(formatter, "{} panicked: {message}", self.source)
            }
            HandlerErrorKind::Parse(error) => {
                write!(formatter, "{} has bad data: {error}", self.source)
            }
        }
    }
}

impl std::error::Error for HandlerError {}
