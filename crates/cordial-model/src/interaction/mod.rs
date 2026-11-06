//! Interactions: the objects that Discord sends when a user uses a command, a component,
//! or a modal, and the responses to them.

mod response;

pub use response::{
    AutocompleteResponse, InteractionCallback, InteractionCallbackResponse,
    InteractionCallbackType, InteractionMessage, InteractionResource, InteractionResponse,
    InteractionResponseData, ModalResponse,
};

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::application::AuthorizingIntegrationOwners;
use crate::channel::Channel;
use crate::command::{CommandOptionType, CommandType};
use crate::component::ComponentType;
use crate::guild::{GuildFeature, Member, PartialMember, Role};
use crate::id::{
    ApplicationId, AttachmentId, ChannelId, CommandId, GenericId, GuildId, InteractionId,
    MessageId, RoleId, UserId,
};
use crate::message::{Attachment, Message};
use crate::monetization::Entitlement;
use crate::user::User;
use crate::{Locale, Nullable, Permissions};

/// An interaction: a command use, a component use, an autocomplete request, or a modal submit.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Interaction {
    /// The interaction ID.
    pub id: InteractionId,
    /// The application ID.
    pub application_id: ApplicationId,
    /// The interaction type.
    #[serde(rename = "type")]
    pub kind: InteractionType,
    /// The data. Absent only for PING.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<InteractionData>,
    /// The partial guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild: Option<InteractionGuild>,
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The partial channel. It always has `id` and `kind`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<Box<Channel>>,
    /// The channel ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<ChannelId>,
    /// The member that used the interaction, with permissions. Only in guilds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Box<Member>>,
    /// The user that used the interaction. Only in DMs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// The token to respond. It is valid for 15 minutes.
    pub token: crate::Secret,
    /// Always 1.
    #[serde(default)]
    pub version: u8,
    /// For components, and modals that a component opened: the message of the component.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<Box<Message>>,
    /// The permissions of the application where the interaction happened.
    #[serde(default)]
    pub app_permissions: Permissions,
    /// The language of the user. Absent only for PING.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<Locale>,
    /// The preferred locale of the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_locale: Option<Locale>,
    /// The entitlements of the user, for monetized applications.
    #[serde(default)]
    pub entitlements: Vec<Entitlement>,
    /// The installation contexts that authorized the interaction.
    #[serde(default)]
    pub authorizing_integration_owners: AuthorizingIntegrationOwners,
    /// Where the interaction happened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<InteractionContextType>,
    /// The upload size limit in bytes.
    #[serde(default)]
    pub attachment_size_limit: u64,
}

impl Interaction {
    /// Gives the user that used the interaction, in a guild or in a DM.
    #[must_use]
    pub fn author(&self) -> Option<&User> {
        self.member
            .as_ref()
            .and_then(|member| member.user.as_ref())
            .or(self.user.as_ref())
    }

    /// Gives the ID of the user that used the interaction.
    #[must_use]
    pub fn author_id(&self) -> Option<UserId> {
        self.author().map(|user| user.id)
    }

    /// Gives the channel ID, from `channel_id` or from `channel`.
    #[must_use]
    pub fn channel_id(&self) -> Option<ChannelId> {
        self.channel_id
            .or_else(|| self.channel.as_ref().map(|channel| channel.id))
    }

    /// Gives the command data, for command and autocomplete interactions.
    #[must_use]
    pub fn command_data(&self) -> Option<&CommandData> {
        match &self.data {
            Some(InteractionData::Command(data)) => Some(data),
            _ => None,
        }
    }

    /// Gives the component data, for component interactions.
    #[must_use]
    pub fn component_data(&self) -> Option<&ComponentData> {
        match &self.data {
            Some(InteractionData::Component(data)) => Some(data),
            _ => None,
        }
    }

    /// Gives the modal submit data, for modal submit interactions.
    #[must_use]
    pub fn modal_data(&self) -> Option<&ModalSubmitData> {
        match &self.data {
            Some(InteractionData::ModalSubmit(data)) => Some(data),
            _ => None,
        }
    }
}

crate::discord_enum! {
    /// The type of an interaction.
    pub enum InteractionType: u8 {
        /// A check from Discord. Only over HTTP.
        Ping = 1,
        /// A command use.
        ApplicationCommand = 2,
        /// A component use.
        MessageComponent = 3,
        /// An autocomplete request.
        ApplicationCommandAutocomplete = 4,
        /// A modal submit.
        ModalSubmit = 5,
    }
}

crate::discord_enum! {
    /// Where an interaction happened, or where a command can be used.
    pub enum InteractionContextType: u8 {
        /// In a guild.
        Guild = 0,
        /// In the DM with the bot of the application.
        BotDm = 1,
        /// In group DMs and DMs other than the DM with the bot.
        PrivateChannel = 2,
    }
}

/// The partial guild of an interaction.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct InteractionGuild {
    /// The guild ID.
    pub id: GuildId,
    /// The features.
    #[serde(default)]
    pub features: Vec<GuildFeature>,
    /// The preferred locale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<Locale>,
}

/// The data of an interaction. The shape depends on the interaction type.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum InteractionData {
    /// Command and autocomplete data.
    Command(Box<CommandData>),
    /// Component data.
    Component(ComponentData),
    /// Modal submit data.
    ModalSubmit(ModalSubmitData),
    /// Data of a shape that this version of the library does not know.
    Unknown(serde_json::Value),
}

/// The data of a command or autocomplete interaction.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct CommandData {
    /// The command ID.
    pub id: CommandId,
    /// The command name.
    pub name: String,
    /// The command type.
    #[serde(rename = "type")]
    pub kind: CommandType,
    /// The users, members, roles, channels, messages, and attachments of the options.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<Resolved>,
    /// The options. For autocomplete, the values can be partial.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<CommandDataOption>,
    /// The guild of a guild command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The target of a user or message command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<GenericId>,
}

impl CommandData {
    /// Gives the options of the subcommand that the user selected, at any depth.
    ///
    /// The second value is the path of subcommand group and subcommand names.
    #[must_use]
    pub fn leaf_options(&self) -> (&[CommandDataOption], Vec<&str>) {
        let mut options = self.options.as_slice();
        let mut path = Vec::new();

        while let [single] = options {
            let is_subcommand = matches!(
                single.kind,
                CommandOptionType::SubCommand | CommandOptionType::SubCommandGroup
            );
            if !is_subcommand {
                break;
            }
            path.push(single.name.as_str());
            options = &single.options;
        }

        (options, path)
    }

    /// Gives the option that has focus, for autocomplete.
    #[must_use]
    pub fn focused_option(&self) -> Option<&CommandDataOption> {
        self.leaf_options()
            .0
            .iter()
            .find(|option| option.focused == Some(true))
    }
}

/// An option value that a user gave in a command.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct CommandDataOption {
    /// The option name.
    pub name: String,
    /// The option type.
    #[serde(rename = "type")]
    pub kind: CommandOptionType,
    /// The value. Absent for subcommands and groups.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<CommandOptionValue>,
    /// The options of a subcommand or a group.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<CommandDataOption>,
    /// For autocomplete: tells if this option has focus.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused: Option<bool>,
}

/// The value of a command option.
///
/// IDs of users, channels, roles, mentionables, and attachments come as strings.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum CommandOptionValue {
    /// A boolean.
    Boolean(bool),
    /// An integer.
    Integer(i64),
    /// A floating point number.
    Number(f64),
    /// A string, or an ID as a string.
    String(String),
}

impl CommandOptionValue {
    /// Gives the value as a string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    /// Gives the value as an integer.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(value) => Some(*value),
            _ => None,
        }
    }

    /// Gives the value as a floating point number. Integers convert too.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(value) => Some(*value),
            Self::Integer(value) => Some(*value as f64),
            _ => None,
        }
    }

    /// Gives the value as a boolean.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    /// Gives the value as an ID, for ID option types.
    #[must_use]
    pub fn as_id<T>(&self) -> Option<crate::Id<T>> {
        self.as_str().and_then(|text| text.parse().ok())
    }
}

/// The data of a component interaction.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ComponentData {
    /// The custom ID of the component.
    pub custom_id: String,
    /// The component type.
    pub component_type: ComponentType,
    /// The selected values of a select menu.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<String>,
    /// The users, members, roles, and channels of an auto-filled select menu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<Resolved>,
    /// The component identifier. The docs show it only in examples.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
}

/// The data of a modal submit interaction.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ModalSubmitData {
    /// The custom ID of the modal.
    pub custom_id: String,
    /// The submitted components. The structure follows the layout of the modal.
    pub components: Vec<SubmittedComponent>,
    /// The users, members, roles, channels, and attachments of the inputs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<Resolved>,
}

impl ModalSubmitData {
    /// Finds a submitted component by custom ID, at any depth.
    #[must_use]
    pub fn find(&self, custom_id: &str) -> Option<&SubmittedComponent> {
        self.components
            .iter()
            .find_map(|component| component.find(custom_id))
    }

    /// Gives the text of a text input, or the value of a radio group.
    #[must_use]
    pub fn text(&self, custom_id: &str) -> Option<&str> {
        match &self.find(custom_id)?.value {
            Nullable::Value(SubmittedValue::String(text)) => Some(text),
            _ => None,
        }
    }

    /// Gives the values of a select menu, a checkbox group, or a file upload.
    #[must_use]
    pub fn values(&self, custom_id: &str) -> Option<&[String]> {
        self.find(custom_id)?.values.as_deref()
    }

    /// Gives the state of a checkbox.
    #[must_use]
    pub fn checked(&self, custom_id: &str) -> Option<bool> {
        match self.find(custom_id)?.value {
            Nullable::Value(SubmittedValue::Boolean(checked)) => Some(checked),
            _ => None,
        }
    }
}

/// A component in a modal submit.
///
/// A label holds its input in `component`. A legacy action row holds inputs in `components`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct SubmittedComponent {
    /// The component type.
    #[serde(rename = "type")]
    pub kind: ComponentType,
    /// The component identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<i32>,
    /// The custom ID of an input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_id: Option<String>,
    /// The value: text for a text input, an optional text for a radio group, a boolean for
    /// a checkbox.
    #[serde(default, skip_serializing_if = "Nullable::is_absent")]
    pub value: Nullable<SubmittedValue>,
    /// The values of a select menu, a checkbox group, or a file upload (attachment IDs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
    /// The input in a label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<Box<SubmittedComponent>>,
    /// The inputs in a legacy action row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<SubmittedComponent>>,
}

impl SubmittedComponent {
    /// Finds a submitted component by custom ID in this component and its children.
    #[must_use]
    pub fn find(&self, custom_id: &str) -> Option<&SubmittedComponent> {
        if self.custom_id.as_deref() == Some(custom_id) {
            return Some(self);
        }

        let in_label = self
            .component
            .as_deref()
            .and_then(|child| child.find(custom_id));
        in_label.or_else(|| {
            self.components
                .iter()
                .flatten()
                .find_map(|child| child.find(custom_id))
        })
    }
}

/// The value of a submitted input.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum SubmittedValue {
    /// The state of a checkbox.
    Boolean(bool),
    /// The text of a text input or the value of a radio group.
    String(String),
}

/// The objects that an interaction refers to, keyed by ID.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Resolved {
    /// The users.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub users: HashMap<UserId, User>,
    /// The partial members. They have no `user`, `deaf`, or `mute`.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub members: HashMap<UserId, PartialMember>,
    /// The roles.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub roles: HashMap<RoleId, Role>,
    /// The partial channels.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub channels: HashMap<ChannelId, Channel>,
    /// The partial messages.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub messages: HashMap<MessageId, Message>,
    /// The attachments.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub attachments: HashMap<AttachmentId, Attachment>,
}
