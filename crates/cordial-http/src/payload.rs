//! The message fields that many endpoints share.

use cordial_model::component::Component;
use cordial_model::embed::Embed;
use cordial_model::message::{AllowedMentions, AttachmentRequest, MessageFlags};
use cordial_model::poll::PollCreate;
use cordial_model::validate;
use serde::Serialize;

use crate::error::Error;
use crate::request::{FileUpload, check};

/// The fields of a message for create, edit, webhook, and interaction requests.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[non_exhaustive]
pub struct MessagePayload {
    /// The text content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Tells if the message is text-to-speech.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tts: Option<bool>,
    /// The embeds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embeds: Option<Vec<Embed>>,
    /// The allowed mentions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_mentions: Option<AllowedMentions>,
    /// The components.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Vec<Component>>,
    /// The attachments: existing ones to keep, and metadata for new files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<AttachmentRequest>>,
    /// The message flags.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
    /// A poll.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll: Option<PollCreate>,
}

impl From<cordial_model::interaction::InteractionMessage> for MessagePayload {
    fn from(message: cordial_model::interaction::InteractionMessage) -> Self {
        Self {
            content: message.content,
            tts: message.tts,
            embeds: message.embeds,
            allowed_mentions: message.allowed_mentions,
            components: message.components,
            attachments: message.attachments,
            flags: message.flags,
            poll: message.poll,
        }
    }
}

impl MessagePayload {
    /// Runs the client-side checks.
    ///
    /// # Errors
    ///
    /// Gives an error when a value breaks a documented limit.
    pub fn validate(&self, validate: bool, files: &[FileUpload]) -> Result<(), Error> {
        check(validate, || {
            if let Some(content) = &self.content {
                validate::message_content(content)?;
            }
            if let Some(embeds) = &self.embeds {
                validate::embeds(embeds)?;
            }
            let flags = self.flags.unwrap_or_default();
            let components = self.components.as_deref().unwrap_or_default();
            let has_content = self
                .content
                .as_deref()
                .is_some_and(|content| !content.is_empty());
            let has_embeds = self
                .embeds
                .as_ref()
                .is_some_and(|embeds| !embeds.is_empty());
            // The docs give no limit for the number of files. Discord answers with an error.
            let _ = files;
            validate::message_components(
                flags,
                components,
                has_content,
                has_embeds,
                self.poll.is_some(),
            )
        })
    }

    /// Adds the attachment entries for new files that have no entry yet.
    pub(crate) fn fill_attachments(&mut self, files: &[FileUpload]) {
        if files.is_empty() {
            return;
        }
        let attachments = self.attachments.get_or_insert_with(Vec::new);
        for (index, file) in files.iter().enumerate() {
            let index = u32::try_from(index).unwrap_or(u32::MAX);
            let present = attachments.iter().any(|attachment| {
                attachment.id == cordial_model::message::AttachmentRequestId::New(index)
            });
            if !present {
                attachments.push(AttachmentRequest::new_file(index, &file.filename));
            }
        }
    }
}

/// Adds the shared message setters to a request builder with fields `payload` and `files`.
macro_rules! message_setters {
    () => {
        /// Sets the text content (up to 2000 characters).
        pub fn content(mut self, content: impl Into<String>) -> Self {
            self.payload.content = Some(content.into());
            self
        }

        /// Sets whether the message is text-to-speech.
        pub fn tts(mut self, tts: bool) -> Self {
            self.payload.tts = Some(tts);
            self
        }

        /// Adds an embed (up to 10).
        pub fn embed(mut self, embed: cordial_model::embed::Embed) -> Self {
            self.payload.embeds.get_or_insert_with(Vec::new).push(embed);
            self
        }

        /// Sets all embeds. An empty list removes the embeds of an existing message.
        pub fn embeds(mut self, embeds: Vec<cordial_model::embed::Embed>) -> Self {
            self.payload.embeds = Some(embeds);
            self
        }

        /// Sets the allowed mentions.
        pub fn allowed_mentions(
            mut self,
            allowed_mentions: cordial_model::message::AllowedMentions,
        ) -> Self {
            self.payload.allowed_mentions = Some(allowed_mentions);
            self
        }

        /// Adds a component.
        pub fn component(
            mut self,
            component: impl Into<cordial_model::component::Component>,
        ) -> Self {
            self.payload
                .components
                .get_or_insert_with(Vec::new)
                .push(component.into());
            self
        }

        /// Sets all components. An empty list removes the components of an existing message.
        pub fn components(mut self, components: Vec<cordial_model::component::Component>) -> Self {
            self.payload.components = Some(components);
            self
        }

        /// Sets the message flags.
        pub fn flags(mut self, flags: cordial_model::message::MessageFlags) -> Self {
            self.payload.flags = Some(flags);
            self
        }

        /// Uses Components V2: the message has only components, no content or embeds.
        pub fn components_v2(mut self) -> Self {
            let flags = self.payload.flags.unwrap_or_default();
            self.payload.flags =
                Some(flags | cordial_model::message::MessageFlags::IS_COMPONENTS_V2);
            self
        }

        /// Sets a poll.
        pub fn poll(mut self, poll: cordial_model::poll::PollCreate) -> Self {
            self.payload.poll = Some(poll);
            self
        }

        /// Adds a file to upload. Refer to it in an embed as `attachment://<filename>`.
        pub fn file(mut self, file: crate::request::FileUpload) -> Self {
            self.files.push(file);
            self
        }

        /// Sets the attachment entries: existing attachments to keep and the metadata of new
        /// files. On edit, attachments that are not listed are removed.
        pub fn attachments(
            mut self,
            attachments: Vec<cordial_model::message::AttachmentRequest>,
        ) -> Self {
            self.payload.attachments = Some(attachments);
            self
        }

        /// Replaces all message fields with a payload. Files stay.
        pub fn payload(mut self, payload: crate::payload::MessagePayload) -> Self {
            self.payload = payload;
            self
        }
    };
}

pub(crate) use message_setters;
