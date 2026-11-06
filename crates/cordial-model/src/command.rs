//! Application commands: slash commands, context menu commands, and entry point commands.

use serde::{Deserialize, Serialize};

use crate::Permissions;
use crate::application::ApplicationIntegrationType;
use crate::channel::ChannelType;
use crate::id::{ApplicationId, CommandId, CommandVersionId, GenericId, GuildId};
use crate::interaction::InteractionContextType;
use crate::locale::Localizations;

/// The maximum number of options of a command or a subcommand.
pub const OPTION_COUNT_LIMIT: usize = 25;
/// The maximum number of choices of an option.
pub const CHOICE_COUNT_LIMIT: usize = 25;
/// The maximum number of characters of all names, descriptions, and values of a command.
pub const TOTAL_CHARACTER_LIMIT: usize = 8000;

/// An application command as Discord stores it.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Command {
    /// The command ID.
    pub id: CommandId,
    /// The command type. The default is `ChatInput`.
    #[serde(rename = "type", default = "chat_input")]
    pub kind: CommandType,
    /// The application ID.
    pub application_id: ApplicationId,
    /// The guild of a guild command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The name, 1 to 32 characters.
    pub name: String,
    /// The localized names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_localizations: Option<Localizations>,
    /// The name in the locale of the request. Only from list endpoints without localizations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_localized: Option<String>,
    /// The description, 1 to 100 characters. Empty for user and message commands.
    #[serde(default)]
    pub description: String,
    /// The localized descriptions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_localizations: Option<Localizations>,
    /// The description in the locale of the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_localized: Option<String>,
    /// The options, up to 25. Only for chat input commands.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<CommandOption>,
    /// The permissions that a member needs by default. `Some(empty)` means administrators only.
    #[serde(default)]
    pub default_member_permissions: Option<Permissions>,
    /// Deprecated: use `contexts`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dm_permission: Option<bool>,
    /// Deprecated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_permission: Option<bool>,
    /// Tells if the command is age-restricted.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub nsfw: bool,
    /// The installation contexts where the command is available. Global commands only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration_types: Option<Vec<ApplicationIntegrationType>>,
    /// The interaction contexts where the command can be used. Global commands only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contexts: Option<Vec<InteractionContextType>>,
    /// The version. It changes when the command changes.
    pub version: CommandVersionId,
    /// For an entry point command: who handles the interaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler: Option<EntryPointHandler>,
}

fn chat_input() -> CommandType {
    CommandType::ChatInput
}

crate::discord_enum! {
    /// The type of an application command.
    pub enum CommandType: u8 {
        /// A slash command.
        ChatInput = 1,
        /// A context menu command on a user.
        User = 2,
        /// A context menu command on a message.
        Message = 3,
        /// The primary entry point of the activity of the application.
        PrimaryEntryPoint = 4,
    }
}

crate::discord_enum! {
    /// Who handles the interaction of an entry point command.
    pub enum EntryPointHandler: u8 {
        /// The application handles it with the interaction token.
        AppHandler = 1,
        /// Discord launches the activity and sends a follow-up message.
        DiscordLaunchActivity = 2,
    }
}

/// A command definition for a create, edit, or bulk overwrite request.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct CommandDefinition {
    /// For a bulk overwrite: the ID of an existing command to update.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<CommandId>,
    /// The command type.
    #[serde(rename = "type", default = "chat_input")]
    pub kind: CommandType,
    /// The name, 1 to 32 characters.
    pub name: String,
    /// The localized names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_localizations: Option<Localizations>,
    /// The description, 1 to 100 characters. Empty for user and message commands.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The localized descriptions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_localizations: Option<Localizations>,
    /// The options, up to 25. Only for chat input commands.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<CommandOption>,
    /// The permissions that a member needs by default. `Some(empty)` means administrators only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_member_permissions: Option<Permissions>,
    /// Tells if the command is age-restricted.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub nsfw: bool,
    /// The installation contexts. Global commands only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration_types: Option<Vec<ApplicationIntegrationType>>,
    /// The interaction contexts. Global commands only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contexts: Option<Vec<InteractionContextType>>,
    /// For an entry point command: who handles the interaction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler: Option<EntryPointHandler>,
}

impl CommandDefinition {
    /// Makes a slash command.
    #[must_use]
    pub fn chat_input(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(CommandType::ChatInput, name, description.into())
    }

    /// Makes a context menu command on users.
    #[must_use]
    pub fn user(name: impl Into<String>) -> Self {
        Self::new(CommandType::User, name, String::new())
    }

    /// Makes a context menu command on messages.
    #[must_use]
    pub fn message(name: impl Into<String>) -> Self {
        Self::new(CommandType::Message, name, String::new())
    }

    fn new(kind: CommandType, name: impl Into<String>, description: String) -> Self {
        Self {
            id: None,
            kind,
            name: name.into(),
            name_localizations: None,
            description,
            description_localizations: None,
            options: Vec::new(),
            default_member_permissions: None,
            nsfw: false,
            integration_types: None,
            contexts: None,
            handler: None,
        }
    }

    /// Adds an option.
    #[must_use]
    pub fn option(mut self, option: CommandOption) -> Self {
        self.options.push(option);
        self
    }

    /// Sets the permissions that a member needs by default.
    #[must_use]
    pub fn default_member_permissions(mut self, permissions: Permissions) -> Self {
        self.default_member_permissions = Some(permissions);
        self
    }

    /// Sets the installation contexts.
    #[must_use]
    pub fn integration_types(
        mut self,
        types: impl IntoIterator<Item = ApplicationIntegrationType>,
    ) -> Self {
        self.integration_types = Some(types.into_iter().collect());
        self
    }

    /// Sets the interaction contexts.
    #[must_use]
    pub fn contexts(mut self, contexts: impl IntoIterator<Item = InteractionContextType>) -> Self {
        self.contexts = Some(contexts.into_iter().collect());
        self
    }

    /// Sets whether the command is age-restricted.
    #[must_use]
    pub fn nsfw(mut self, nsfw: bool) -> Self {
        self.nsfw = nsfw;
        self
    }

    /// Adds a localized name and description.
    #[must_use]
    pub fn localize(
        mut self,
        locale: crate::Locale,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        self.name_localizations
            .get_or_insert_with(Localizations::new)
            .insert(locale.clone(), name.into());
        let description = description.into();
        if !description.is_empty() {
            self.description_localizations
                .get_or_insert_with(Localizations::new)
                .insert(locale, description);
        }
        self
    }
}

impl From<&Command> for CommandDefinition {
    fn from(command: &Command) -> Self {
        Self {
            id: Some(command.id),
            kind: command.kind,
            name: command.name.clone(),
            name_localizations: command.name_localizations.clone(),
            description: command.description.clone(),
            description_localizations: command.description_localizations.clone(),
            options: command.options.clone(),
            default_member_permissions: command.default_member_permissions,
            nsfw: command.nsfw,
            integration_types: command.integration_types.clone(),
            contexts: command.contexts.clone(),
            handler: command.handler,
        }
    }
}

/// An option of a command: a parameter, a subcommand, or a subcommand group.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct CommandOption {
    /// The option type.
    #[serde(rename = "type")]
    pub kind: CommandOptionType,
    /// The name, 1 to 32 characters, unique among its siblings.
    pub name: String,
    /// The localized names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_localizations: Option<Localizations>,
    /// The description, 1 to 100 characters.
    pub description: String,
    /// The localized descriptions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_localizations: Option<Localizations>,
    /// Tells if the option is required. Required options must come first.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub required: bool,
    /// Fixed choices, up to 25. Only for string, integer, and number options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<CommandOptionChoice>>,
    /// The options of a subcommand or a group.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<CommandOption>,
    /// For channel options: the channel types to show.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_types: Option<Vec<ChannelType>>,
    /// The minimum value of an integer or number option.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_value: Option<NumberLimit>,
    /// The maximum value of an integer or number option.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_value: Option<NumberLimit>,
    /// The minimum length of a string option, 0 to 6000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<u16>,
    /// The maximum length of a string option, 1 to 6000.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<u16>,
    /// Tells if the option uses autocomplete. Not with `choices`.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub autocomplete: bool,
    /// For attachment options: the file types, up to 10 (`image`, `video`, `audio`, `.ext`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_types: Option<Vec<String>>,
}

impl CommandOption {
    /// Makes an option.
    #[must_use]
    pub fn new(
        kind: CommandOptionType,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            name: name.into(),
            name_localizations: None,
            description: description.into(),
            description_localizations: None,
            required: false,
            choices: None,
            options: Vec::new(),
            channel_types: None,
            min_value: None,
            max_value: None,
            min_length: None,
            max_length: None,
            autocomplete: false,
            file_types: None,
        }
    }

    /// Sets whether the option is required.
    #[must_use]
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Adds a choice.
    #[must_use]
    pub fn choice(
        mut self,
        name: impl Into<String>,
        value: impl Into<CommandOptionChoiceValue>,
    ) -> Self {
        self.choices
            .get_or_insert_with(Vec::new)
            .push(CommandOptionChoice::new(name, value));
        self
    }

    /// Adds a nested option (for subcommands and groups).
    #[must_use]
    pub fn option(mut self, option: CommandOption) -> Self {
        self.options.push(option);
        self
    }

    /// Sets whether the option uses autocomplete.
    #[must_use]
    pub fn autocomplete(mut self, autocomplete: bool) -> Self {
        self.autocomplete = autocomplete;
        self
    }
}

crate::discord_enum! {
    /// The type of a command option.
    pub enum CommandOptionType: u8 {
        /// A subcommand.
        SubCommand = 1,
        /// A group of subcommands.
        SubCommandGroup = 2,
        /// A string.
        String = 3,
        /// An integer from -(2^53 - 1) to 2^53 - 1.
        Integer = 4,
        /// A boolean.
        Boolean = 5,
        /// A user.
        User = 6,
        /// A channel (all types, categories included).
        Channel = 7,
        /// A role.
        Role = 8,
        /// A user or a role.
        Mentionable = 9,
        /// A double from -2^53 to 2^53.
        Number = 10,
        /// An attachment.
        Attachment = 11,
    }
}

/// A minimum or maximum value: an integer for integer options, a double for number options.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum NumberLimit {
    /// An integer limit.
    Integer(i64),
    /// A double limit.
    Number(f64),
}

/// A fixed choice of an option, or an autocomplete suggestion.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct CommandOptionChoice {
    /// The name, 1 to 100 characters.
    pub name: String,
    /// The localized names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_localizations: Option<Localizations>,
    /// The value.
    pub value: CommandOptionChoiceValue,
}

impl CommandOptionChoice {
    /// Makes a choice.
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<CommandOptionChoiceValue>) -> Self {
        Self {
            name: name.into(),
            name_localizations: None,
            value: value.into(),
        }
    }
}

/// The value of a choice.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum CommandOptionChoiceValue {
    /// An integer.
    Integer(i64),
    /// A double.
    Number(f64),
    /// A string, up to 100 characters.
    String(String),
}

impl From<i64> for CommandOptionChoiceValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<f64> for CommandOptionChoiceValue {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}

impl From<&str> for CommandOptionChoiceValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<String> for CommandOptionChoiceValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

/// The permissions of commands in a guild.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildCommandPermissions {
    /// The command ID, or the application ID for the default of all commands.
    pub id: GenericId,
    /// The application ID.
    pub application_id: ApplicationId,
    /// The guild ID.
    pub guild_id: GuildId,
    /// The permissions, up to 100.
    pub permissions: Vec<CommandPermission>,
}

/// One permission of a command in a guild.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct CommandPermission {
    /// The role, user, or channel. The guild ID means `@everyone`; the guild ID minus 1
    /// means all channels.
    pub id: GenericId,
    /// The target type.
    #[serde(rename = "type")]
    pub kind: CommandPermissionType,
    /// `true` allows, `false` denies.
    pub permission: bool,
}

impl CommandPermission {
    /// Makes a permission for a role, a user, or a channel.
    #[must_use]
    pub fn new(id: GenericId, kind: CommandPermissionType, permission: bool) -> Self {
        Self {
            id,
            kind,
            permission,
        }
    }
}

crate::discord_enum! {
    /// The target type of a command permission.
    pub enum CommandPermissionType: u8 {
        /// A role.
        Role = 1,
        /// A user.
        User = 2,
        /// A channel.
        Channel = 3,
    }
}

/// The ID of the constant "all channels" for command permissions: the guild ID minus 1.
#[must_use]
pub fn all_channels_permission_id(guild_id: GuildId) -> Option<GenericId> {
    GenericId::new(guild_id.get() - 1)
}
