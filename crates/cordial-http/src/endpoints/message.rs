//! Message endpoints.

use std::collections::HashSet;

use cordial_model::Timestamp;
use cordial_model::channel::{Channel, ThreadMember};
use cordial_model::id::{ChannelId, GuildId, MessageId, RoleId, StickerId, UserId};
use cordial_model::message::{Message, MessagePins, MessageReference, Nonce};
use cordial_model::validate::ValidationError;
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::Error;
use crate::payload::{MessagePayload, message_setters};
use crate::request::{Endpoint, FileUpload, RequestBody, check};
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

/// `POST /channels/{channel.id}/messages/{message.id}/crosspost`: publishes a message of an
/// announcement channel to the following channels.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CrosspostMessage {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
}

impl Endpoint for CrosspostMessage {
    type Response = Message;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/channels/{channel_id}/messages/{message_id}/crosspost",
            format!(
                "/channels/{}/messages/{}/crosspost",
                self.channel_id, self.message_id
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// The smallest number of messages in a bulk delete.
const BULK_DELETE_MIN: usize = 2;
/// The largest number of messages in a bulk delete.
const BULK_DELETE_MAX: usize = 100;

/// `POST /channels/{channel.id}/messages/bulk-delete`: deletes 2 to 100 messages.
///
/// Discord refuses the request when a message is older than 2 weeks.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct BulkDeleteMessages {
    client: Client,
    channel_id: ChannelId,
    message_ids: Vec<MessageId>,
    reason: Option<String>,
}

#[derive(Serialize)]
struct BulkDeleteBody<'a> {
    messages: &'a [MessageId],
}

impl BulkDeleteMessages {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for BulkDeleteMessages {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/channels/{channel_id}/messages/bulk-delete",
            format!("/channels/{}/messages/bulk-delete", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            let count = self.message_ids.len();
            if !(BULK_DELETE_MIN..=BULK_DELETE_MAX).contains(&count) {
                return Err(ValidationError::new(
                    "messages",
                    format!("has {count} message IDs; the range is 2 to 100"),
                ));
            }
            let mut seen = HashSet::with_capacity(count);
            if !self.message_ids.iter().all(|id| seen.insert(*id)) {
                return Err(ValidationError::new(
                    "messages",
                    "has the same message ID more than one time",
                ));
            }
            Ok(())
        })?;

        RequestBody::json(&BulkDeleteBody {
            messages: &self.message_ids,
        })
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /channels/{channel.id}/messages/pins`: the pinned messages of a channel, newest pin
/// first.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetChannelPins {
    client: Client,
    channel_id: ChannelId,
    before: Option<Timestamp>,
    limit: Option<u8>,
}

impl GetChannelPins {
    /// Gets messages pinned before this time. Use the `pinned_at` of the last pin to get the
    /// next page.
    pub fn before(mut self, pinned_at: Timestamp) -> Self {
        self.before = Some(pinned_at);
        self
    }

    /// Sets the number of pins, 1 to 50. The default is 50.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 50));
        self
    }
}

impl Endpoint for GetChannelPins {
    type Response = MessagePins;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("before", self.before);
        query.push_opt("limit", self.limit);
        Route::new(
            Method::Get,
            "/channels/{channel_id}/messages/pins",
            format!(
                "/channels/{}/messages/pins{}",
                self.channel_id,
                query.finish()
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `PUT /channels/{channel.id}/messages/pins/{message.id}`: pins a message.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct PinMessage {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
    reason: Option<String>,
}

impl PinMessage {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for PinMessage {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/channels/{channel_id}/messages/pins/{message_id}",
            format!(
                "/channels/{}/messages/pins/{}",
                self.channel_id, self.message_id
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /channels/{channel.id}/messages/pins/{message.id}`: unpins a message.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct UnpinMessage {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
    reason: Option<String>,
}

impl UnpinMessage {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for UnpinMessage {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/channels/{channel_id}/messages/pins/{message_id}",
            format!(
                "/channels/{}/messages/pins/{}",
                self.channel_id, self.message_id
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /channels/{channel.id}/pins` (deprecated): the first 50 pinned messages of a channel.
///
/// Use [`GetChannelPins`] instead.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetPinnedMessagesLegacy {
    client: Client,
    channel_id: ChannelId,
}

impl Endpoint for GetPinnedMessagesLegacy {
    type Response = Vec<Message>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/channels/{channel_id}/pins",
            format!("/channels/{}/pins", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `PUT /channels/{channel.id}/pins/{message.id}` (deprecated): pins a message.
///
/// Use [`PinMessage`] instead.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct PinMessageLegacy {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
}

impl Endpoint for PinMessageLegacy {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/channels/{channel_id}/pins/{message_id}",
            format!("/channels/{}/pins/{}", self.channel_id, self.message_id),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `DELETE /channels/{channel.id}/pins/{message.id}` (deprecated): unpins a message.
///
/// Use [`UnpinMessage`] instead.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct UnpinMessageLegacy {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
}

impl Endpoint for UnpinMessageLegacy {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/channels/{channel_id}/pins/{message_id}",
            format!("/channels/{}/pins/{}", self.channel_id, self.message_id),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// Defines a search filter enum that the query string gives as text.
macro_rules! search_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident = $text:literal,)*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum $name {
            $($(#[$variant_meta])* $variant,)*
        }

        impl $name {
            /// Gives the value as the query string has it.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)*
                }
            }
        }
    };
}

search_enum! {
    /// The type of the author of a message, for a message search.
    pub enum SearchAuthorType {
        /// A user account.
        User = "user",
        /// A bot account.
        Bot = "bot",
        /// A webhook.
        Webhook = "webhook",
    }
}

search_enum! {
    /// A thing that a message has, for a message search.
    pub enum SearchHas {
        /// An image.
        Image = "image",
        /// A sound attachment.
        Sound = "sound",
        /// A video.
        Video = "video",
        /// An attachment.
        File = "file",
        /// A sticker.
        Sticker = "sticker",
        /// An embed.
        Embed = "embed",
        /// A link.
        Link = "link",
        /// A poll.
        Poll = "poll",
        /// A forwarded message.
        Snapshot = "snapshot",
    }
}

search_enum! {
    /// A type of embed, for a message search. The values are wider than the embed types.
    pub enum SearchEmbedType {
        /// An image embed.
        Image = "image",
        /// A video embed.
        Video = "video",
        /// A GIF (`gifv`) embed.
        Gif = "gif",
        /// A sound embed.
        Sound = "sound",
        /// An article embed.
        Article = "article",
    }
}

search_enum! {
    /// The sort mode of a message search.
    pub enum SearchSortBy {
        /// By the creation time of the message (the default).
        Timestamp = "timestamp",
        /// By the relevance to the search query. The sort order has no effect.
        Relevance = "relevance",
    }
}

search_enum! {
    /// The sort direction of a message search.
    pub enum SearchSortOrder {
        /// Oldest first.
        Ascending = "asc",
        /// Newest first (the default).
        Descending = "desc",
    }
}

/// The largest offset of a message search.
const SEARCH_OFFSET_MAX: u16 = 9975;
/// The largest number of channels in a message search.
const SEARCH_CHANNEL_MAX: usize = 500;
/// The largest number of values in most list filters of a message search.
const SEARCH_LIST_MAX: usize = 100;
/// The largest length of the content filter, in characters.
const SEARCH_CONTENT_MAX: usize = 1024;
/// The largest length of a link host name, an embed provider, or a file extension filter.
const SEARCH_SHORT_TEXT_MAX: usize = 256;

/// `GET /guilds/{guild.id}/messages/search`: messages of a guild that match the filters.
///
/// The response has no `reactions`. When Discord has not indexed the guild yet, the response
/// is [`MessageSearch::NotIndexed`]. Then send the request again after `retry_after`.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct SearchGuildMessages {
    client: Client,
    guild_id: GuildId,
    limit: Option<u8>,
    offset: Option<u16>,
    max_id: Option<MessageId>,
    min_id: Option<MessageId>,
    slop: Option<u8>,
    content: Option<String>,
    channel_ids: Vec<ChannelId>,
    author_types: Vec<String>,
    author_ids: Vec<UserId>,
    mentions: Vec<UserId>,
    mentions_role_ids: Vec<RoleId>,
    mention_everyone: Option<bool>,
    replied_to_user_ids: Vec<UserId>,
    replied_to_message_ids: Vec<MessageId>,
    pinned: Option<bool>,
    has: Vec<String>,
    embed_types: Vec<SearchEmbedType>,
    embed_providers: Vec<String>,
    link_hostnames: Vec<String>,
    attachment_filenames: Vec<String>,
    attachment_extensions: Vec<String>,
    sort_by: Option<SearchSortBy>,
    sort_order: Option<SearchSortOrder>,
    include_nsfw: Option<bool>,
}

impl SearchGuildMessages {
    /// Sets the number of messages, 1 to 25. The default is 25.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 25));
        self
    }

    /// Skips this number of results (up to 9975).
    pub fn offset(mut self, offset: u16) -> Self {
        self.offset = Some(offset.min(SEARCH_OFFSET_MAX));
        self
    }

    /// Gets messages before this message ID.
    pub fn max_id(mut self, message_id: MessageId) -> Self {
        self.max_id = Some(message_id);
        self
    }

    /// Gets messages after this message ID.
    pub fn min_id(mut self, message_id: MessageId) -> Self {
        self.min_id = Some(message_id);
        self
    }

    /// Sets the largest number of words between matching words of the content (up to 100).
    /// The default is 2.
    pub fn slop(mut self, slop: u8) -> Self {
        self.slop = Some(slop.min(100));
        self
    }

    /// Filters by content (up to 1024 characters).
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Filters by channel. Call it more than one time for more channels (up to 500).
    pub fn channel_id(mut self, channel_id: ChannelId) -> Self {
        self.channel_ids.push(channel_id);
        self
    }

    /// Includes only messages from this type of author.
    pub fn author_type(mut self, author_type: SearchAuthorType) -> Self {
        self.author_types.push(author_type.as_str().to_owned());
        self
    }

    /// Excludes messages from this type of author.
    pub fn exclude_author_type(mut self, author_type: SearchAuthorType) -> Self {
        self.author_types.push(format!("-{}", author_type.as_str()));
        self
    }

    /// Filters by author. Call it more than one time for more authors (up to 100).
    pub fn author_id(mut self, user_id: UserId) -> Self {
        self.author_ids.push(user_id);
        self
    }

    /// Filters by a mentioned user (up to 100).
    pub fn mentions(mut self, user_id: UserId) -> Self {
        self.mentions.push(user_id);
        self
    }

    /// Filters by a mentioned role (up to 100).
    pub fn mentions_role_id(mut self, role_id: RoleId) -> Self {
        self.mentions_role_ids.push(role_id);
        self
    }

    /// Filters by whether the message mentions `@everyone`.
    pub fn mention_everyone(mut self, mention_everyone: bool) -> Self {
        self.mention_everyone = Some(mention_everyone);
        self
    }

    /// Filters by replies to a user (up to 100).
    pub fn replied_to_user_id(mut self, user_id: UserId) -> Self {
        self.replied_to_user_ids.push(user_id);
        self
    }

    /// Filters by replies to a message (up to 100).
    pub fn replied_to_message_id(mut self, message_id: MessageId) -> Self {
        self.replied_to_message_ids.push(message_id);
        self
    }

    /// Filters by whether the message is pinned.
    pub fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = Some(pinned);
        self
    }

    /// Includes only messages that have this thing.
    pub fn has(mut self, has: SearchHas) -> Self {
        self.has.push(has.as_str().to_owned());
        self
    }

    /// Excludes messages that have this thing.
    pub fn has_not(mut self, has: SearchHas) -> Self {
        self.has.push(format!("-{}", has.as_str()));
        self
    }

    /// Filters by embed type.
    pub fn embed_type(mut self, embed_type: SearchEmbedType) -> Self {
        self.embed_types.push(embed_type);
        self
    }

    /// Filters by embed provider, for example `Tenor`. The value is case-sensitive.
    pub fn embed_provider(mut self, provider: impl Into<String>) -> Self {
        self.embed_providers.push(provider.into());
        self
    }

    /// Filters by the host name of a link, for example `discordapp.com`.
    pub fn link_hostname(mut self, hostname: impl Into<String>) -> Self {
        self.link_hostnames.push(hostname.into());
        self
    }

    /// Filters by attachment file name (up to 1024 characters).
    pub fn attachment_filename(mut self, filename: impl Into<String>) -> Self {
        self.attachment_filenames.push(filename.into());
        self
    }

    /// Filters by attachment extension, for example `txt`.
    pub fn attachment_extension(mut self, extension: impl Into<String>) -> Self {
        self.attachment_extensions.push(extension.into());
        self
    }

    /// Sets the sort mode.
    pub fn sort_by(mut self, sort_by: SearchSortBy) -> Self {
        self.sort_by = Some(sort_by);
        self
    }

    /// Sets the sort direction. Discord ignores it when the sort mode is relevance.
    pub fn sort_order(mut self, sort_order: SearchSortOrder) -> Self {
        self.sort_order = Some(sort_order);
        self
    }

    /// Includes results from age-restricted channels. The default is `false`.
    pub fn include_nsfw(mut self, include_nsfw: bool) -> Self {
        self.include_nsfw = Some(include_nsfw);
        self
    }

    fn check_filters(&self) -> Result<(), ValidationError> {
        if let Some(content) = &self.content {
            check_text_length("content", content, SEARCH_CONTENT_MAX)?;
        }
        check_list_count("channel_id", self.channel_ids.len(), SEARCH_CHANNEL_MAX)?;
        let lists = [
            ("author_id", self.author_ids.len()),
            ("mentions", self.mentions.len()),
            ("mentions_role_id", self.mentions_role_ids.len()),
            ("replied_to_user_id", self.replied_to_user_ids.len()),
            ("replied_to_message_id", self.replied_to_message_ids.len()),
            ("embed_provider", self.embed_providers.len()),
            ("link_hostname", self.link_hostnames.len()),
            ("attachment_filename", self.attachment_filenames.len()),
            ("attachment_extension", self.attachment_extensions.len()),
        ];
        for (field, count) in lists {
            check_list_count(field, count, SEARCH_LIST_MAX)?;
        }
        let texts = [
            (
                "embed_provider",
                &self.embed_providers,
                SEARCH_SHORT_TEXT_MAX,
            ),
            ("link_hostname", &self.link_hostnames, SEARCH_SHORT_TEXT_MAX),
            (
                "attachment_extension",
                &self.attachment_extensions,
                SEARCH_SHORT_TEXT_MAX,
            ),
            (
                "attachment_filename",
                &self.attachment_filenames,
                SEARCH_CONTENT_MAX,
            ),
        ];
        for (field, values, max) in texts {
            for value in values {
                check_text_length(field, value, max)?;
            }
        }
        Ok(())
    }
}

fn check_list_count(field: &str, count: usize, max: usize) -> Result<(), ValidationError> {
    if count > max {
        return Err(ValidationError::new(
            field,
            format!("has {count} values; the maximum is {max}"),
        ));
    }
    Ok(())
}

fn check_text_length(field: &str, text: &str, max: usize) -> Result<(), ValidationError> {
    let length = text.chars().count();
    if length > max {
        return Err(ValidationError::new(
            field,
            format!("has {length} characters; the maximum is {max}"),
        ));
    }
    Ok(())
}

impl Endpoint for SearchGuildMessages {
    type Response = MessageSearch;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("limit", self.limit);
        query.push_opt("offset", self.offset);
        query.push_opt("max_id", self.max_id);
        query.push_opt("min_id", self.min_id);
        query.push_opt("slop", self.slop);
        query.push_opt("content", self.content.as_deref());
        for channel_id in &self.channel_ids {
            query.push("channel_id", channel_id);
        }
        for author_type in &self.author_types {
            query.push("author_type", author_type);
        }
        for user_id in &self.author_ids {
            query.push("author_id", user_id);
        }
        for user_id in &self.mentions {
            query.push("mentions", user_id);
        }
        for role_id in &self.mentions_role_ids {
            query.push("mentions_role_id", role_id);
        }
        query.push_opt("mention_everyone", self.mention_everyone);
        for user_id in &self.replied_to_user_ids {
            query.push("replied_to_user_id", user_id);
        }
        for message_id in &self.replied_to_message_ids {
            query.push("replied_to_message_id", message_id);
        }
        query.push_opt("pinned", self.pinned);
        for has in &self.has {
            query.push("has", has);
        }
        for embed_type in &self.embed_types {
            query.push("embed_type", embed_type.as_str());
        }
        for provider in &self.embed_providers {
            query.push("embed_provider", provider);
        }
        for hostname in &self.link_hostnames {
            query.push("link_hostname", hostname);
        }
        for filename in &self.attachment_filenames {
            query.push("attachment_filename", filename);
        }
        for extension in &self.attachment_extensions {
            query.push("attachment_extension", extension);
        }
        query.push_opt("sort_by", self.sort_by.map(SearchSortBy::as_str));
        query.push_opt("sort_order", self.sort_order.map(SearchSortOrder::as_str));
        query.push_opt("include_nsfw", self.include_nsfw);
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/messages/search",
            format!(
                "/guilds/{}/messages/search{}",
                self.guild_id,
                query.finish()
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        // A GET request has no body. The filters are checked here because this runs before
        // the request is sent.
        check(validate, || self.check_filters())?;
        Ok(RequestBody::None)
    }
}

/// The response of a message search.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum MessageSearch {
    /// The results (status 200).
    Results(MessageSearchResults),
    /// The guild is not indexed yet (status 202). Send the request again later.
    NotIndexed(SearchIndexNotReady),
}

/// The results of a message search.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessageSearchResults {
    /// Tells if the guild has a deep historical index operation in progress.
    #[serde(default)]
    pub doing_deep_historical_index: bool,
    /// The number of documents that the current index operation indexed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub documents_indexed: Option<u64>,
    /// The total number of matching messages. It can be wrong while messages change.
    pub total_results: u64,
    /// The matching messages. Each inner list holds one message: Discord no longer sends the
    /// surrounding messages.
    pub messages: Vec<Vec<Message>>,
    /// The threads that hold the messages.
    #[serde(default)]
    pub threads: Vec<Channel>,
    /// The thread members of the current user, for the threads that it joined.
    #[serde(default)]
    pub members: Vec<ThreadMember>,
}

impl MessageSearchResults {
    /// Gives the matching messages as one list.
    pub fn iter_messages(&self) -> impl Iterator<Item = &Message> {
        self.messages.iter().flatten()
    }
}

/// The response of a message search when the guild is not indexed yet.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct SearchIndexNotReady {
    /// The message, for example "Index not yet available. Try again later".
    #[serde(default)]
    pub message: String,
    /// The code. It is 110000.
    pub code: u32,
    /// The number of documents that Discord indexed.
    #[serde(default)]
    pub documents_indexed: u64,
    /// The time to wait in seconds. With 0, wait a short time.
    pub retry_after: f64,
}

super::into_future!(
    GetMessages,
    GetMessage,
    CreateMessage,
    EditMessage,
    DeleteMessage,
    CrosspostMessage,
    BulkDeleteMessages,
    GetChannelPins,
    PinMessage,
    UnpinMessage,
    GetPinnedMessagesLegacy,
    PinMessageLegacy,
    UnpinMessageLegacy,
    SearchGuildMessages,
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

    /// Publishes a message of an announcement channel to the following channels.
    pub fn crosspost_message(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> CrosspostMessage {
        CrosspostMessage {
            client: self.clone(),
            channel_id,
            message_id,
        }
    }

    /// Deletes 2 to 100 messages that are not older than 2 weeks.
    pub fn bulk_delete_messages(
        &self,
        channel_id: ChannelId,
        message_ids: Vec<MessageId>,
    ) -> BulkDeleteMessages {
        BulkDeleteMessages {
            client: self.clone(),
            channel_id,
            message_ids,
            reason: None,
        }
    }

    /// Gets the pinned messages of a channel, newest pin first.
    pub fn channel_pins(&self, channel_id: ChannelId) -> GetChannelPins {
        GetChannelPins {
            client: self.clone(),
            channel_id,
            before: None,
            limit: None,
        }
    }

    /// Pins a message.
    pub fn pin_message(&self, channel_id: ChannelId, message_id: MessageId) -> PinMessage {
        PinMessage {
            client: self.clone(),
            channel_id,
            message_id,
            reason: None,
        }
    }

    /// Unpins a message.
    pub fn unpin_message(&self, channel_id: ChannelId, message_id: MessageId) -> UnpinMessage {
        UnpinMessage {
            client: self.clone(),
            channel_id,
            message_id,
            reason: None,
        }
    }

    /// Gets the first 50 pinned messages of a channel, with the deprecated route.
    #[deprecated(note = "Discord deprecated this route; use `channel_pins`")]
    pub fn pinned_messages_legacy(&self, channel_id: ChannelId) -> GetPinnedMessagesLegacy {
        GetPinnedMessagesLegacy {
            client: self.clone(),
            channel_id,
        }
    }

    /// Pins a message, with the deprecated route.
    #[deprecated(note = "Discord deprecated this route; use `pin_message`")]
    pub fn pin_message_legacy(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> PinMessageLegacy {
        PinMessageLegacy {
            client: self.clone(),
            channel_id,
            message_id,
        }
    }

    /// Unpins a message, with the deprecated route.
    #[deprecated(note = "Discord deprecated this route; use `unpin_message`")]
    pub fn unpin_message_legacy(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
    ) -> UnpinMessageLegacy {
        UnpinMessageLegacy {
            client: self.clone(),
            channel_id,
            message_id,
        }
    }

    /// Searches the messages of a guild. Add filters with the setters.
    pub fn search_guild_messages(&self, guild_id: GuildId) -> SearchGuildMessages {
        SearchGuildMessages {
            client: self.clone(),
            guild_id,
            limit: None,
            offset: None,
            max_id: None,
            min_id: None,
            slop: None,
            content: None,
            channel_ids: Vec::new(),
            author_types: Vec::new(),
            author_ids: Vec::new(),
            mentions: Vec::new(),
            mentions_role_ids: Vec::new(),
            mention_everyone: None,
            replied_to_user_ids: Vec::new(),
            replied_to_message_ids: Vec::new(),
            pinned: None,
            has: Vec::new(),
            embed_types: Vec::new(),
            embed_providers: Vec::new(),
            link_hostnames: Vec::new(),
            attachment_filenames: Vec::new(),
            attachment_extensions: Vec::new(),
            sort_by: None,
            sort_order: None,
            include_nsfw: None,
        }
    }
}
