//! Typed application commands.
//!
//! The derive macros implement these traits. The same macro input gives the definition that
//! goes to Discord and the parser for the interaction data. Thus the two cannot disagree.
//!
//! | Trait | Derive | Use |
//! |---|---|---|
//! | [`CommandOptions`] | `Command`, `CommandGroup` | A set of options, or a set of subcommands. |
//! | [`Command`] | `Command`, `CommandGroup` with `#[command(name = ...)]` | A top-level command. |
//! | [`CommandArg`] | `Choice` | The type of one option. |
//! | [`CommandTarget`] | none | The target of a user or message command. |

mod arg;

pub use arg::{CommandArg, Mentionable, ResolvedUser};

use std::fmt;

use cordial_model::command::{CommandDefinition, CommandOptionType, CommandType};
use cordial_model::interaction::{CommandData, CommandDataOption, Resolved};
use cordial_model::message::Message;

/// A set of options of a command or a subcommand, or a set of subcommands.
pub trait CommandOptions: Sized {
    /// Gives the option definitions.
    fn options() -> Vec<cordial_model::command::CommandOption>;

    /// Parses the options of an interaction.
    ///
    /// # Errors
    ///
    /// Gives an error when the data does not match the definition.
    fn parse(input: OptionsInput<'_>) -> Result<Self, ParseError>;
}

/// A top-level command: a slash command, a user command, or a message command.
///
/// `#[derive(Command)]` and `#[derive(CommandGroup)]` implement it. Give the handler of the
/// command to [`BotBuilder::command`](crate::BotBuilder::command).
///
/// ```
/// use cordial_framework::Command;
/// use cordial_model::command::CommandType;
///
/// /// Bans a member.
/// #[derive(Command)]
/// #[command(name = "ban", description = "Ban a member", default_member_permissions = "BAN_MEMBERS")]
/// struct Ban {
///     #[option(description = "The reason, for the audit log", max_length = 512)]
///     reason: Option<String>,
/// }
///
/// assert_eq!(Ban::NAME, "ban");
/// assert_eq!(Ban::KIND, CommandType::ChatInput);
/// let definition = Ban::definition();
/// assert_eq!(definition.options[0].name, "reason");
/// assert!(!definition.options[0].required);
/// ```
pub trait Command: Sized + Send + 'static {
    /// The command name. The router uses it with the command type.
    const NAME: &'static str;

    /// The command type.
    const KIND: CommandType;

    /// Gives the definition that goes to Discord.
    fn definition() -> CommandDefinition;

    /// Parses the data of a command interaction.
    ///
    /// # Errors
    ///
    /// Gives an error when the data does not match the definition.
    fn parse(data: &CommandData) -> Result<Self, ParseError>;
}

/// The target of a user command or a message command.
pub trait CommandTarget: Sized {
    /// The command type that has this target.
    const KIND: CommandType;

    /// Gets the target from the interaction data.
    ///
    /// # Errors
    ///
    /// Gives an error when the data has no resolved target.
    fn from_data(data: &CommandData) -> Result<Self, ParseError>;
}

impl CommandTarget for ResolvedUser {
    const KIND: CommandType = CommandType::User;

    fn from_data(data: &CommandData) -> Result<Self, ParseError> {
        let target_id = data
            .target_id
            .ok_or(ParseError::new(ParseErrorKind::MissingTarget))?;
        ResolvedUser::resolve(target_id.cast(), data.resolved.as_ref())
    }
}

impl CommandTarget for Message {
    const KIND: CommandType = CommandType::Message;

    fn from_data(data: &CommandData) -> Result<Self, ParseError> {
        let target_id = data
            .target_id
            .ok_or(ParseError::new(ParseErrorKind::MissingTarget))?;
        data.resolved
            .as_ref()
            .and_then(|resolved| resolved.messages.get(&target_id.cast()))
            .cloned()
            .ok_or(ParseError::new(ParseErrorKind::Unresolved))
    }
}

/// The options of one level of a command interaction, with the resolved objects.
#[derive(Clone, Copy, Debug)]
pub struct OptionsInput<'a> {
    options: &'a [CommandDataOption],
    resolved: Option<&'a Resolved>,
}

impl<'a> OptionsInput<'a> {
    /// Makes an input from options and resolved objects.
    #[must_use]
    pub fn new(options: &'a [CommandDataOption], resolved: Option<&'a Resolved>) -> Self {
        Self { options, resolved }
    }

    /// Makes the input of the top level of a command interaction.
    #[must_use]
    pub fn from_data(data: &'a CommandData) -> Self {
        Self::new(&data.options, data.resolved.as_ref())
    }

    /// Gives the resolved objects.
    #[must_use]
    pub fn resolved(&self) -> Option<&'a Resolved> {
        self.resolved
    }

    /// Finds an option by name.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&'a CommandDataOption> {
        self.options.iter().find(|option| option.name == name)
    }

    /// Parses a required option.
    ///
    /// # Errors
    ///
    /// Gives an error when the option is absent or has the wrong type.
    pub fn required<T: CommandArg>(&self, name: &str) -> Result<T, ParseError> {
        self.optional(name)?
            .ok_or_else(|| ParseError::new(ParseErrorKind::Missing).at(name))
    }

    /// Parses an optional option.
    ///
    /// Autocomplete interactions can send a partial value of the wrong type. Use
    /// [`OptionsInput::focused`] to read the focused value.
    ///
    /// # Errors
    ///
    /// Gives an error when the option has the wrong type.
    pub fn optional<T: CommandArg>(&self, name: &str) -> Result<Option<T>, ParseError> {
        let Some(option) = self.get(name) else {
            return Ok(None);
        };
        let value = option
            .value
            .as_ref()
            .ok_or_else(|| ParseError::new(ParseErrorKind::Missing).at(name))?;
        T::parse(value, self.resolved)
            .map(Some)
            .map_err(|error| error.at(name))
    }

    /// Gives the selected subcommand or subcommand group, and its options.
    ///
    /// # Errors
    ///
    /// Gives an error when this level has no subcommand.
    pub fn subcommand(&self) -> Result<(&'a str, OptionsInput<'a>), ParseError> {
        self.options
            .iter()
            .find(|option| {
                matches!(
                    option.kind,
                    CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
                )
            })
            .map(|option| {
                (
                    option.name.as_str(),
                    OptionsInput::new(&option.options, self.resolved),
                )
            })
            .ok_or(ParseError::new(ParseErrorKind::MissingSubcommand))
    }

    /// Gives the focused option of an autocomplete interaction, at any level.
    #[must_use]
    pub fn focused(&self) -> Option<&'a CommandDataOption> {
        focused_in(self.options)
    }
}

fn focused_in(options: &[CommandDataOption]) -> Option<&CommandDataOption> {
    options.iter().find_map(|option| {
        if option.focused == Some(true) {
            Some(option)
        } else {
            focused_in(&option.options)
        }
    })
}

/// The reason that the interaction data does not match a command definition.
///
/// This happens when the definition on Discord is older than the code. Sync the commands.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParseError {
    /// The path of option names to the problem, outermost first.
    pub path: Vec<String>,
    /// The problem.
    pub kind: ParseErrorKind,
}

/// The problem in a [`ParseError`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseErrorKind {
    /// A required option is absent.
    Missing,
    /// The option value has a different type.
    WrongType {
        /// The expected option type.
        expected: CommandOptionType,
    },
    /// The value is out of the range of the Rust type.
    OutOfRange,
    /// The resolved object for an ID is absent.
    Unresolved,
    /// The value is not one of the choices.
    UnknownChoice(String),
    /// The subcommand is not known.
    UnknownSubcommand(String),
    /// The interaction has no subcommand.
    MissingSubcommand,
    /// A user or message command has no target.
    MissingTarget,
    /// A modal field is absent.
    MissingField,
    /// Another problem.
    Other(String),
}

impl ParseError {
    /// Makes an error with an empty path.
    #[must_use]
    pub fn new(kind: ParseErrorKind) -> Self {
        Self {
            path: Vec::new(),
            kind,
        }
    }

    /// Adds an outer option name to the path.
    #[must_use]
    pub fn at(mut self, name: &str) -> Self {
        self.path.insert(0, name.to_owned());
        self
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.path.is_empty() {
            write!(formatter, "option `{}`: ", self.path.join(" "))?;
        }
        match &self.kind {
            ParseErrorKind::Missing => formatter.write_str("the value is absent"),
            ParseErrorKind::WrongType { expected } => {
                write!(formatter, "the value is not of type {expected:?}")
            }
            ParseErrorKind::OutOfRange => formatter.write_str("the value is out of range"),
            ParseErrorKind::Unresolved => formatter.write_str("the resolved object is absent"),
            ParseErrorKind::UnknownChoice(value) => write!(formatter, "unknown choice `{value}`"),
            ParseErrorKind::UnknownSubcommand(name) => {
                write!(formatter, "unknown subcommand `{name}`")
            }
            ParseErrorKind::MissingSubcommand => formatter.write_str("the subcommand is absent"),
            ParseErrorKind::MissingTarget => formatter.write_str("the command target is absent"),
            ParseErrorKind::MissingField => formatter.write_str("the modal field is absent"),
            ParseErrorKind::Other(message) => formatter.write_str(message),
        }?;
        formatter.write_str(" (the command definition on Discord may be out of date)")
    }
}

impl std::error::Error for ParseError {}
