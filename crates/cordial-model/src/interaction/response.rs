//! Responses to interactions.

use serde::{Deserialize, Serialize};

use crate::command::CommandOptionChoice;
use crate::component::Component;
use crate::embed::Embed;
use crate::id::{InteractionId, MessageId};
use crate::message::{AllowedMentions, AttachmentRequest, Message, MessageFlags};
use crate::poll::PollCreate;

use super::InteractionType;

crate::discord_enum! {
    /// The type of an interaction response.
    pub enum InteractionCallbackType: u8 {
        /// Acknowledge a PING.
        Pong = 1,
        /// Respond with a message.
        ChannelMessageWithSource = 4,
        /// Acknowledge now and send a message later. The user sees a loading state.
        DeferredChannelMessageWithSource = 5,
        /// For components and modal submits: acknowledge now and edit the message later.
        DeferredUpdateMessage = 6,
        /// For components and modal submits: edit the message of the component.
        UpdateMessage = 7,
        /// Respond to an autocomplete request with choices.
        ApplicationCommandAutocompleteResult = 8,
        /// Respond with a modal. Not valid for modal submits.
        Modal = 9,
        /// Deprecated: use a premium button.
        PremiumRequired = 10,
        /// Launch the activity of the application.
        LaunchActivity = 12,
    }
}

/// A response to an interaction.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct InteractionResponse {
    /// The response type.
    #[serde(rename = "type")]
    pub kind: InteractionCallbackType,
    /// The response data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<InteractionResponseData>,
}

impl InteractionResponse {
    /// Acknowledges a PING.
    #[must_use]
    pub const fn pong() -> Self {
        Self {
            kind: InteractionCallbackType::Pong,
            data: None,
        }
    }

    /// Responds with a message.
    #[must_use]
    pub fn message(message: InteractionMessage) -> Self {
        Self {
            kind: InteractionCallbackType::ChannelMessageWithSource,
            data: Some(InteractionResponseData::Message(Box::new(message))),
        }
    }

    /// Acknowledges now and sends a message later. Only the ephemeral flag is valid here.
    #[must_use]
    pub fn defer(ephemeral: bool) -> Self {
        let data = ephemeral.then(|| {
            InteractionResponseData::Message(Box::new(InteractionMessage {
                flags: Some(MessageFlags::EPHEMERAL),
                ..InteractionMessage::default()
            }))
        });
        Self {
            kind: InteractionCallbackType::DeferredChannelMessageWithSource,
            data,
        }
    }

    /// For components and modal submits: acknowledges now and edits the message later.
    #[must_use]
    pub const fn defer_update() -> Self {
        Self {
            kind: InteractionCallbackType::DeferredUpdateMessage,
            data: None,
        }
    }

    /// For components and modal submits: edits the message of the component.
    #[must_use]
    pub fn update(message: InteractionMessage) -> Self {
        Self {
            kind: InteractionCallbackType::UpdateMessage,
            data: Some(InteractionResponseData::Message(Box::new(message))),
        }
    }

    /// Responds to an autocomplete request.
    #[must_use]
    pub fn autocomplete(choices: Vec<CommandOptionChoice>) -> Self {
        Self {
            kind: InteractionCallbackType::ApplicationCommandAutocompleteResult,
            data: Some(InteractionResponseData::Autocomplete(
                AutocompleteResponse { choices },
            )),
        }
    }

    /// Responds with a modal.
    #[must_use]
    pub fn modal(modal: ModalResponse) -> Self {
        Self {
            kind: InteractionCallbackType::Modal,
            data: Some(InteractionResponseData::Modal(modal)),
        }
    }

    /// Launches the activity of the application.
    #[must_use]
    pub const fn launch_activity() -> Self {
        Self {
            kind: InteractionCallbackType::LaunchActivity,
            data: None,
        }
    }
}

/// The data of an interaction response.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum InteractionResponseData {
    /// Choices for an autocomplete request.
    Autocomplete(AutocompleteResponse),
    /// A modal.
    Modal(ModalResponse),
    /// A message.
    Message(Box<InteractionMessage>),
}

/// A message in an interaction response.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct InteractionMessage {
    /// Tells if the message is text-to-speech.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tts: Option<bool>,
    /// The text content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// The embeds, up to 10.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embeds: Option<Vec<Embed>>,
    /// The allowed mentions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_mentions: Option<AllowedMentions>,
    /// The flags: only `SUPPRESS_EMBEDS`, `EPHEMERAL`, `IS_COMPONENTS_V2`, `IS_VOICE_MESSAGE`,
    /// and `SUPPRESS_NOTIFICATIONS` are valid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
    /// The components.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<Component>>,
    /// The attachments. The files go in the multipart body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<AttachmentRequest>>,
    /// A poll.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poll: Option<PollCreate>,
}

impl From<&str> for InteractionMessage {
    fn from(content: &str) -> Self {
        Self::text(content)
    }
}

impl From<String> for InteractionMessage {
    fn from(content: String) -> Self {
        Self::text(content)
    }
}

impl From<Embed> for InteractionMessage {
    fn from(embed: Embed) -> Self {
        Self::new().embed(embed)
    }
}

impl InteractionMessage {
    /// Makes an empty message.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes a message with text content.
    #[must_use]
    pub fn text(content: impl Into<String>) -> Self {
        Self::new().content(content)
    }

    /// Sets the text content.
    #[must_use]
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Adds an embed (up to 10).
    #[must_use]
    pub fn embed(mut self, embed: Embed) -> Self {
        self.embeds.get_or_insert_with(Vec::new).push(embed);
        self
    }

    /// Adds a component.
    #[must_use]
    pub fn component(mut self, component: impl Into<Component>) -> Self {
        self.components
            .get_or_insert_with(Vec::new)
            .push(component.into());
        self
    }

    /// Sets the allowed mentions.
    #[must_use]
    pub fn allowed_mentions(mut self, allowed_mentions: AllowedMentions) -> Self {
        self.allowed_mentions = Some(allowed_mentions);
        self
    }

    /// Adds message flags.
    #[must_use]
    pub fn flags(mut self, flags: MessageFlags) -> Self {
        self.flags = Some(self.flags.unwrap_or_default() | flags);
        self
    }

    /// Makes the message visible only to the user of the interaction.
    #[must_use]
    pub fn ephemeral(self) -> Self {
        self.flags(MessageFlags::EPHEMERAL)
    }

    /// Uses Components V2: the message has only components, no content or embeds.
    #[must_use]
    pub fn components_v2(self) -> Self {
        self.flags(MessageFlags::IS_COMPONENTS_V2)
    }

    /// Sets the attachment entries for files in a multipart request.
    #[must_use]
    pub fn attachments(mut self, attachments: Vec<AttachmentRequest>) -> Self {
        self.attachments = Some(attachments);
        self
    }

    /// Sets a poll.
    #[must_use]
    pub fn poll(mut self, poll: PollCreate) -> Self {
        self.poll = Some(poll);
        self
    }

    /// Sets whether the message is text-to-speech.
    #[must_use]
    pub fn tts(mut self, tts: bool) -> Self {
        self.tts = Some(tts);
        self
    }
}

/// The choices of an autocomplete response.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AutocompleteResponse {
    /// The choices, up to 25.
    pub choices: Vec<CommandOptionChoice>,
}

/// A modal in an interaction response.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ModalResponse {
    /// The custom ID, 1 to 100 characters.
    pub custom_id: String,
    /// The title, up to 45 characters.
    pub title: String,
    /// The components, 1 to 5: labels and text displays.
    pub components: Vec<Component>,
}

impl ModalResponse {
    /// Makes a modal.
    #[must_use]
    pub fn new(
        custom_id: impl Into<String>,
        title: impl Into<String>,
        components: Vec<Component>,
    ) -> Self {
        Self {
            custom_id: custom_id.into(),
            title: title.into(),
            components,
        }
    }
}

/// The response of the interaction callback endpoint with `with_response=true`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct InteractionCallbackResponse {
    /// The interaction.
    pub interaction: InteractionCallback,
    /// The resource that the response made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<InteractionResource>,
}

/// The interaction in a callback response.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct InteractionCallback {
    /// The interaction ID.
    pub id: InteractionId,
    /// The interaction type.
    #[serde(rename = "type")]
    pub kind: InteractionType,
    /// The activity instance, if one was launched or joined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_instance_id: Option<String>,
    /// The message that the response made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_message_id: Option<MessageId>,
    /// Tells if the message is in a loading state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_message_loading: Option<bool>,
    /// Tells if the message is ephemeral.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_message_ephemeral: Option<bool>,
}

/// The resource that an interaction response made.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct InteractionResource {
    /// The response type.
    #[serde(rename = "type")]
    pub kind: InteractionCallbackType,
    /// The activity instance, for `LaunchActivity`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_instance: Option<ActivityInstanceResource>,
    /// The message, for message responses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<Box<Message>>,
}

/// The activity instance of an interaction response.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ActivityInstanceResource {
    /// The instance ID.
    pub id: String,
}
