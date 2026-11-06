//! Message endpoints.

use cordial_model::id::{ChannelId, MessageId, StickerId};
use cordial_model::message::{Message, MessageReference, Nonce};
use serde::Serialize;

use crate::client::Client;
use crate::error::Error;
use crate::payload::{MessagePayload, message_setters};
use crate::request::{Endpoint, FileUpload, RequestBody};
use crate::route::{MajorParameter, Method, Query, Route};

/// `GET /channels/{channel.id}/messages`: messages of a channel, newest first.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetMessages {
    client: Client,
    channel_id: ChannelId,
    position: Option<(&'static str, MessageId)>,
    limit: Option<u8>,
}

impl GetMessages {
    /// Gets messages before a message.
    pub fn before(mut self, message_id: MessageId) -> Self {
        self.position = Some(("before", message_id));
        self
    }

    /// Gets messages after a message.
    pub fn after(mut self, message_id: MessageId) -> Self {
        self.position = Some(("after", message_id));
        self
    }

    /// Gets messages around a message.
    pub fn around(mut self, message_id: MessageId) -> Self {
        self.position = Some(("around", message_id));
        self
    }

    /// Sets the number of messages, 1 to 100. The default is 50.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 100));
        self
    }
}

impl Endpoint for GetMessages {
    type Response = Vec<Message>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        if let Some((name, message_id)) = self.position {
            query.push(name, message_id);
        }
        query.push_opt("limit", self.limit);
        Route::new(
            Method::Get,
            "/channels/{channel_id}/messages",
            format!("/channels/{}/messages{}", self.channel_id, query.finish()),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `GET /channels/{channel.id}/messages/{message.id}`: one message.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetMessage {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
}

impl Endpoint for GetMessage {
    type Response = Message;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/channels/{channel_id}/messages/{message_id}",
            format!("/channels/{}/messages/{}", self.channel_id, self.message_id),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `POST /channels/{channel.id}/messages`: sends a message.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateMessage {
    client: Client,
    channel_id: ChannelId,
    payload: MessagePayload,
    extra: CreateMessageExtra,
    files: Vec<FileUpload>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct CreateMessageExtra {
    #[serde(skip_serializing_if = "Option::is_none")]
    nonce: Option<Nonce>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enforce_nonce: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_reference: Option<MessageReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sticker_ids: Option<Vec<StickerId>>,
}

#[derive(Serialize)]
struct CreateMessageBody<'a> {
    #[serde(flatten)]
    payload: &'a MessagePayload,
    #[serde(flatten)]
    extra: &'a CreateMessageExtra,
}

impl CreateMessage {
    message_setters!();

    /// Replies to a message.
    pub fn reply_to(mut self, message_id: MessageId) -> Self {
        self.extra.message_reference = Some(MessageReference::reply(message_id));
        self
    }

    /// Sets the message reference, for example a forward.
    pub fn message_reference(mut self, reference: MessageReference) -> Self {
        self.extra.message_reference = Some(reference);
        self
    }

    /// Forwards a message. The new message has no other content.
    pub fn forward(mut self, channel_id: ChannelId, message_id: MessageId) -> Self {
        self.extra.message_reference = Some(MessageReference::forward(channel_id, message_id));
        self
    }

    /// Adds stickers (up to 3).
    pub fn stickers(mut self, sticker_ids: Vec<StickerId>) -> Self {
        self.extra.sticker_ids = Some(sticker_ids);
        self
    }

    /// Sets a nonce. With `enforce`, Discord sends no duplicate for the same nonce within a
    /// few minutes. That also makes the request safe to retry.
    pub fn nonce(mut self, nonce: impl Into<String>, enforce: bool) -> Self {
        self.extra.nonce = Some(Nonce::String(nonce.into()));
        self.extra.enforce_nonce = Some(enforce);
        self
    }
}

impl Endpoint for CreateMessage {
    type Response = Message;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/channels/{channel_id}/messages",
            format!("/channels/{}/messages", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        self.payload.validate(validate, &self.files)?;
        crate::request::check(validate, || {
            if let Some(stickers) = &self.extra.sticker_ids
                && stickers.len() > cordial_model::message::STICKER_COUNT_LIMIT
            {
                return Err(cordial_model::validate::ValidationError::new(
                    "sticker_ids",
                    "has more than 3 stickers",
                ));
            }
            let empty = self.payload.content.as_deref().is_none_or(str::is_empty)
                && self.payload.embeds.as_ref().is_none_or(Vec::is_empty)
                && self.payload.components.as_ref().is_none_or(Vec::is_empty)
                && self.payload.poll.is_none()
                && self.extra.sticker_ids.as_ref().is_none_or(Vec::is_empty)
                && self.files.is_empty()
                && self.extra.message_reference.is_none();
            if empty {
                return Err(cordial_model::validate::ValidationError::new(
                    "message",
                    "needs content, embeds, components, a poll, stickers, files, or a forward",
                ));
            }
            Ok(())
        })?;

        let mut payload = self.payload.clone();
        payload.fill_attachments(&self.files);
        let body = CreateMessageBody {
            payload: &payload,
            extra: &self.extra,
        };
        RequestBody::json_with_files(&body, &self.files)
    }

    fn retry_safe(&self) -> bool {
        self.extra.enforce_nonce == Some(true)
    }
}

/// `PATCH /channels/{channel.id}/messages/{message.id}`: edits a message.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditMessage {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
    payload: MessagePayload,
    files: Vec<FileUpload>,
}

impl EditMessage {
    message_setters!();

    /// Removes the content.
    pub fn clear_content(mut self) -> Self {
        self.payload.content = Some(String::new());
        self
    }
}

impl Endpoint for EditMessage {
    type Response = Message;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/channels/{channel_id}/messages/{message_id}",
            format!("/channels/{}/messages/{}", self.channel_id, self.message_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        self.payload.validate(validate, &self.files)?;
        let mut payload = self.payload.clone();
        payload.fill_attachments(&self.files);
        RequestBody::json_with_files(&payload, &self.files)
    }
}

/// `DELETE /channels/{channel.id}/messages/{message.id}`: deletes a message.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteMessage {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
    reason: Option<String>,
}

impl DeleteMessage {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for DeleteMessage {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/channels/{channel_id}/messages/{message_id}",
            format!("/channels/{}/messages/{}", self.channel_id, self.message_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

super::into_future!(
    GetMessages,
    GetMessage,
    CreateMessage,
    EditMessage,
    DeleteMessage
);

impl Client {
    /// Gets messages of a channel, newest first.
    pub fn messages(&self, channel_id: ChannelId) -> GetMessages {
        GetMessages {
            client: self.clone(),
            channel_id,
            position: None,
            limit: None,
        }
    }

    /// Gets one message.
    pub fn message(&self, channel_id: ChannelId, message_id: MessageId) -> GetMessage {
        GetMessage {
            client: self.clone(),
            channel_id,
            message_id,
        }
    }

    /// Sends a message.
    pub fn create_message(&self, channel_id: ChannelId) -> CreateMessage {
        CreateMessage {
            client: self.clone(),
            channel_id,
            payload: MessagePayload::default(),
            extra: CreateMessageExtra::default(),
            files: Vec::new(),
        }
    }

    /// Edits a message.
    pub fn edit_message(&self, channel_id: ChannelId, message_id: MessageId) -> EditMessage {
        EditMessage {
            client: self.clone(),
            channel_id,
            message_id,
            payload: MessagePayload::default(),
            files: Vec::new(),
        }
    }

    /// Deletes a message.
    pub fn delete_message(&self, channel_id: ChannelId, message_id: MessageId) -> DeleteMessage {
        DeleteMessage {
            client: self.clone(),
            channel_id,
            message_id,
            reason: None,
        }
    }
}
