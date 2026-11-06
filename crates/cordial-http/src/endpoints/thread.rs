//! Thread endpoints: start threads, thread members, archived threads, and thread search.

use cordial_model::Timestamp;
use cordial_model::channel::{AutoArchiveDuration, Channel, ChannelType, ThreadList, ThreadMember};
use cordial_model::id::{ChannelId, ForumTagId, MessageId, StickerId, UserId};
use cordial_model::message::{Message, STICKER_COUNT_LIMIT};
use cordial_model::validate::ValidationError;
use serde::{Deserialize, Deserializer, Serialize};

use super::channel::{check_count, check_length, check_rate_limit};
use crate::client::Client;
use crate::error::Error;
use crate::payload::{MessagePayload, message_setters};
use crate::request::{Endpoint, FileUpload, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route};

/// The maximum number of tags on a thread.
const APPLIED_TAG_LIMIT: usize = 5;

/// A thread in a forum or media channel, with its first message.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ForumThread {
    /// The thread.
    #[serde(flatten)]
    pub channel: Channel,
    /// The first message of the thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
}

/// A page of thread search results.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ThreadSearchResults {
    /// The threads that match.
    pub threads: Vec<Channel>,
    /// The thread members of the current user, for the threads that the user joined.
    pub members: Vec<ThreadMember>,
    /// Tells if more results are available.
    pub has_more: bool,
    /// The first message of each thread, when Discord has it.
    #[serde(default)]
    pub first_messages: Vec<Message>,
    /// The total number of threads that match.
    pub total_results: u32,
}

/// The answer to a search while Discord still builds the search index (status 202).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct SearchIndexNotReady {
    /// The message.
    pub message: String,
    /// The error code.
    pub code: u32,
    /// The number of documents in the index.
    pub documents_indexed: u64,
    /// The time to wait before the next try, in seconds.
    pub retry_after: u32,
}

/// The answer to a thread search.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum ThreadSearchResponse {
    /// The results.
    Results(ThreadSearchResults),
    /// The search index is not ready. Try again after `retry_after` seconds.
    IndexNotReady(SearchIndexNotReady),
}

impl<'de> Deserialize<'de> for ThreadSearchResponse {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Look at the keys first, so that a decode error names the real problem.
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.get("documents_indexed").is_some() && value.get("threads").is_none() {
            serde_json::from_value(value)
                .map(Self::IndexNotReady)
                .map_err(serde::de::Error::custom)
        } else {
            serde_json::from_value(value)
                .map(Self::Results)
                .map_err(serde::de::Error::custom)
        }
    }
}

/// How a thread search matches the tags of the query.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ThreadSearchTagMode {
    /// The thread has all tags of the query.
    MatchAll,
    /// The thread has one or more tags of the query.
    MatchSome,
}

impl ThreadSearchTagMode {
    /// Gives the value for the query string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MatchAll => "match_all",
            Self::MatchSome => "match_some",
        }
    }
}

/// The sort key of a thread search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ThreadSearchSort {
    /// Sort by relevance to the name query.
    Relevance,
    /// Sort by the creation time.
    CreationTime,
    /// Sort by the time of the last message.
    LastMessageTime,
    /// Sort by the archive time.
    ArchiveTime,
}

impl ThreadSearchSort {
    /// Gives the value for the query string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::CreationTime => "creation_time",
            Self::LastMessageTime => "last_message_time",
            Self::ArchiveTime => "archive_time",
        }
    }
}

/// The sort direction of a search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SearchOrder {
    /// The smallest value first.
    Ascending,
    /// The largest value first.
    Descending,
}

impl SearchOrder {
    /// Gives the value for the query string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ascending => "asc",
            Self::Descending => "desc",
        }
    }
}

/// The fields that the three "start thread" endpoints share.
#[derive(Clone, Debug, Default, Serialize)]
struct ThreadFields {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_archive_duration: Option<AutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit_per_user: Option<u32>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<ChannelType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invitable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    applied_tags: Option<Vec<ForumTagId>>,
}

impl ThreadFields {
    fn new(name: String) -> Self {
        Self {
            name,
            ..Self::default()
        }
    }

    fn validate(&self) -> Result<(), ValidationError> {
        check_length("name", &self.name, 1, 100)?;
        check_rate_limit("rate_limit_per_user", self.rate_limit_per_user)?;
        if let Some(tags) = &self.applied_tags {
            check_count("applied_tags", tags.len(), APPLIED_TAG_LIMIT)?;
        }
        Ok(())
    }
}

/// `POST /channels/{channel.id}/messages/{message.id}/threads`: starts a thread from a
/// message.
///
/// The thread has the same ID as the message.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateThreadFromMessage {
    client: Client,
    channel_id: ChannelId,
    message_id: MessageId,
    fields: ThreadFields,
    reason: Option<String>,
}

impl CreateThreadFromMessage {
    /// Sets the time of inactivity after which the thread is archived.
    pub fn auto_archive_duration(mut self, duration: AutoArchiveDuration) -> Self {
        self.fields.auto_archive_duration = Some(duration);
        self
    }

    /// Sets the slowmode in seconds (0 to 21600).
    pub fn rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.fields.rate_limit_per_user = Some(seconds);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for CreateThreadFromMessage {
    type Response = Channel;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/channels/{channel_id}/messages/{message_id}/threads",
            format!(
                "/channels/{}/messages/{}/threads",
                self.channel_id, self.message_id
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.fields.validate())?;
        RequestBody::json(&self.fields)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `POST /channels/{channel.id}/threads`: starts a thread that has no start message.
///
/// The default type is a private thread. Use [`CreateThread::kind`] for a public thread.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateThread {
    client: Client,
    channel_id: ChannelId,
    fields: ThreadFields,
    reason: Option<String>,
}

impl CreateThread {
    /// Sets the thread type: `PublicThread`, `PrivateThread`, or `AnnouncementThread`.
    pub fn kind(mut self, kind: ChannelType) -> Self {
        self.fields.kind = Some(kind);
        self
    }

    /// Sets whether members that are not moderators can add others to a private thread.
    pub fn invitable(mut self, invitable: bool) -> Self {
        self.fields.invitable = Some(invitable);
        self
    }

    /// Sets the time of inactivity after which the thread is archived.
    pub fn auto_archive_duration(mut self, duration: AutoArchiveDuration) -> Self {
        self.fields.auto_archive_duration = Some(duration);
        self
    }

    /// Sets the slowmode in seconds (0 to 21600).
    pub fn rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.fields.rate_limit_per_user = Some(seconds);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for CreateThread {
    type Response = Channel;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/channels/{channel_id}/threads",
            format!("/channels/{}/threads", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.fields.validate())?;
        RequestBody::json(&self.fields)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `POST /channels/{channel.id}/threads`: starts a thread (a post) in a forum or media
/// channel, with a first message.
///
/// The message setters set the first message. The thread is always a public thread.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateForumThread {
    client: Client,
    channel_id: ChannelId,
    fields: ThreadFields,
    payload: MessagePayload,
    sticker_ids: Option<Vec<StickerId>>,
    files: Vec<FileUpload>,
    reason: Option<String>,
}

#[derive(Serialize)]
struct ForumThreadBody<'a> {
    #[serde(flatten)]
    fields: &'a ThreadFields,
    message: ForumMessageBody<'a>,
}

#[derive(Serialize)]
struct ForumMessageBody<'a> {
    #[serde(flatten)]
    payload: &'a MessagePayload,
    #[serde(skip_serializing_if = "Option::is_none")]
    sticker_ids: Option<&'a Vec<StickerId>>,
}

impl CreateForumThread {
    message_setters!();

    /// Adds stickers to the first message (up to 3).
    pub fn stickers(mut self, sticker_ids: Vec<StickerId>) -> Self {
        self.sticker_ids = Some(sticker_ids);
        self
    }

    /// Sets the tags of the thread (up to 5).
    pub fn applied_tags(mut self, tags: Vec<ForumTagId>) -> Self {
        self.fields.applied_tags = Some(tags);
        self
    }

    /// Sets the time of inactivity after which the thread is archived.
    pub fn auto_archive_duration(mut self, duration: AutoArchiveDuration) -> Self {
        self.fields.auto_archive_duration = Some(duration);
        self
    }

    /// Sets the slowmode in seconds (0 to 21600).
    pub fn rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.fields.rate_limit_per_user = Some(seconds);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    fn validate_message(&self) -> Result<(), ValidationError> {
        if let Some(stickers) = &self.sticker_ids {
            check_count("sticker_ids", stickers.len(), STICKER_COUNT_LIMIT)?;
        }
        let empty = self.payload.content.as_deref().is_none_or(str::is_empty)
            && self.payload.embeds.as_ref().is_none_or(Vec::is_empty)
            && self.payload.components.as_ref().is_none_or(Vec::is_empty)
            && self.sticker_ids.as_ref().is_none_or(Vec::is_empty)
            && self.files.is_empty();
        if empty {
            return Err(ValidationError::new(
                "message",
                "needs content, embeds, components, stickers, or files",
            ));
        }
        Ok(())
    }
}

impl Endpoint for CreateForumThread {
    type Response = ForumThread;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/channels/{channel_id}/threads",
            format!("/channels/{}/threads", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.fields.validate())?;
        self.payload.validate(validate, &self.files)?;
        check(validate, || self.validate_message())?;

        let mut payload = self.payload.clone();
        payload.fill_attachments(&self.files);
        let body = ForumThreadBody {
            fields: &self.fields,
            message: ForumMessageBody {
                payload: &payload,
                sticker_ids: self.sticker_ids.as_ref(),
            },
        };
        RequestBody::json_with_files(&body, &self.files)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `PUT /channels/{channel.id}/thread-members/@me`: adds the current user to a thread.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct JoinThread {
    client: Client,
    channel_id: ChannelId,
}

impl Endpoint for JoinThread {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/channels/{channel_id}/thread-members/@me",
            format!("/channels/{}/thread-members/@me", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `DELETE /channels/{channel.id}/thread-members/@me`: removes the current user from a
/// thread.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct LeaveThread {
    client: Client,
    channel_id: ChannelId,
}

impl Endpoint for LeaveThread {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/channels/{channel_id}/thread-members/@me",
            format!("/channels/{}/thread-members/@me", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `PUT /channels/{channel.id}/thread-members/{user.id}`: adds a member to a thread.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct AddThreadMember {
    client: Client,
    channel_id: ChannelId,
    user_id: UserId,
}

impl Endpoint for AddThreadMember {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/channels/{channel_id}/thread-members/{user_id}",
            format!(
                "/channels/{}/thread-members/{}",
                self.channel_id, self.user_id
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `DELETE /channels/{channel.id}/thread-members/{user.id}`: removes a member from a
/// thread.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct RemoveThreadMember {
    client: Client,
    channel_id: ChannelId,
    user_id: UserId,
}

impl Endpoint for RemoveThreadMember {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/channels/{channel_id}/thread-members/{user_id}",
            format!(
                "/channels/{}/thread-members/{}",
                self.channel_id, self.user_id
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `GET /channels/{channel.id}/thread-members/{user.id}`: one member of a thread.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetThreadMember {
    client: Client,
    channel_id: ChannelId,
    user_id: UserId,
    with_member: Option<bool>,
}

impl GetThreadMember {
    /// Includes the guild member in the response.
    pub fn with_member(mut self, with_member: bool) -> Self {
        self.with_member = Some(with_member);
        self
    }
}

impl Endpoint for GetThreadMember {
    type Response = ThreadMember;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("with_member", self.with_member);
        Route::new(
            Method::Get,
            "/channels/{channel_id}/thread-members/{user_id}",
            format!(
                "/channels/{}/thread-members/{}{}",
                self.channel_id,
                self.user_id,
                query.finish()
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `GET /channels/{channel.id}/thread-members`: the members of a thread.
///
/// This needs the `GUILD_MEMBERS` privileged intent. With `with_member`, the results are
/// paginated.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetThreadMembers {
    client: Client,
    channel_id: ChannelId,
    with_member: Option<bool>,
    after: Option<UserId>,
    limit: Option<u8>,
}

impl GetThreadMembers {
    /// Includes the guild member of each thread member, and turns on pagination.
    pub fn with_member(mut self, with_member: bool) -> Self {
        self.with_member = Some(with_member);
        self
    }

    /// Gets thread members after this user ID.
    pub fn after(mut self, user_id: UserId) -> Self {
        self.after = Some(user_id);
        self
    }

    /// Sets the number of thread members, 1 to 100. The default is 100.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 100));
        self
    }
}

impl Endpoint for GetThreadMembers {
    type Response = Vec<ThreadMember>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("with_member", self.with_member);
        query.push_opt("after", self.after);
        query.push_opt("limit", self.limit);
        Route::new(
            Method::Get,
            "/channels/{channel_id}/thread-members",
            format!(
                "/channels/{}/thread-members{}",
                self.channel_id,
                query.finish()
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `GET /channels/{channel.id}/threads/archived/public`: archived public threads, newest
/// archive first.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetPublicArchivedThreads {
    client: Client,
    channel_id: ChannelId,
    before: Option<Timestamp>,
    limit: Option<u8>,
}

impl GetPublicArchivedThreads {
    /// Gets threads that were archived before this time.
    pub fn before(mut self, before: Timestamp) -> Self {
        self.before = Some(before);
        self
    }

    /// Sets the maximum number of threads, 2 to 100.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(2, 100));
        self
    }
}

impl Endpoint for GetPublicArchivedThreads {
    type Response = ThreadList;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("before", self.before);
        query.push_opt("limit", self.limit);
        Route::new(
            Method::Get,
            "/channels/{channel_id}/threads/archived/public",
            format!(
                "/channels/{}/threads/archived/public{}",
                self.channel_id,
                query.finish()
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `GET /channels/{channel.id}/threads/archived/private`: archived private threads, newest
/// archive first.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetPrivateArchivedThreads {
    client: Client,
    channel_id: ChannelId,
    before: Option<Timestamp>,
    limit: Option<u8>,
}

impl GetPrivateArchivedThreads {
    /// Gets threads that were archived before this time.
    pub fn before(mut self, before: Timestamp) -> Self {
        self.before = Some(before);
        self
    }

    /// Sets the maximum number of threads, 2 to 100.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(2, 100));
        self
    }
}

impl Endpoint for GetPrivateArchivedThreads {
    type Response = ThreadList;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("before", self.before);
        query.push_opt("limit", self.limit);
        Route::new(
            Method::Get,
            "/channels/{channel_id}/threads/archived/private",
            format!(
                "/channels/{}/threads/archived/private{}",
                self.channel_id,
                query.finish()
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `GET /channels/{channel.id}/users/@me/threads/archived/private`: archived private
/// threads that the current user joined, highest ID first.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetJoinedPrivateArchivedThreads {
    client: Client,
    channel_id: ChannelId,
    before: Option<ChannelId>,
    limit: Option<u8>,
}

impl GetJoinedPrivateArchivedThreads {
    /// Gets threads with an ID before this thread ID.
    pub fn before(mut self, thread_id: ChannelId) -> Self {
        self.before = Some(thread_id);
        self
    }

    /// Sets the maximum number of threads, 2 to 100.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(2, 100));
        self
    }
}

impl Endpoint for GetJoinedPrivateArchivedThreads {
    type Response = ThreadList;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("before", self.before);
        query.push_opt("limit", self.limit);
        Route::new(
            Method::Get,
            "/channels/{channel_id}/users/@me/threads/archived/private",
            format!(
                "/channels/{}/users/@me/threads/archived/private{}",
                self.channel_id,
                query.finish()
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `GET /channels/{channel.id}/threads/search`: searches the threads of a channel.
///
/// The Discord docs do not describe this endpoint. The OpenAPI specification gives its
/// parameters.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct SearchThreads {
    client: Client,
    channel_id: ChannelId,
    name: Option<String>,
    slop: Option<u8>,
    min_id: Option<ChannelId>,
    max_id: Option<ChannelId>,
    tags: Vec<ForumTagId>,
    tag_mode: Option<ThreadSearchTagMode>,
    archived: Option<bool>,
    sort_by: Option<ThreadSearchSort>,
    sort_order: Option<SearchOrder>,
    limit: Option<u8>,
    offset: Option<u16>,
}

impl SearchThreads {
    /// Searches for threads with this name (up to 100 characters).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the number of words that can be between the query words in a match, 0 to 100.
    pub fn slop(mut self, slop: u8) -> Self {
        self.slop = Some(slop.min(100));
        self
    }

    /// Gets threads with an ID greater than this ID.
    pub fn min_id(mut self, thread_id: ChannelId) -> Self {
        self.min_id = Some(thread_id);
        self
    }

    /// Gets threads with an ID less than this ID.
    pub fn max_id(mut self, thread_id: ChannelId) -> Self {
        self.max_id = Some(thread_id);
        self
    }

    /// Adds a tag to match (up to 20 tags).
    pub fn tag(mut self, tag_id: ForumTagId) -> Self {
        self.tags.push(tag_id);
        self
    }

    /// Sets how the search matches the tags.
    pub fn tag_mode(mut self, mode: ThreadSearchTagMode) -> Self {
        self.tag_mode = Some(mode);
        self
    }

    /// Searches only archived threads (`true`) or only active threads (`false`).
    pub fn archived(mut self, archived: bool) -> Self {
        self.archived = Some(archived);
        self
    }

    /// Sets the sort key.
    pub fn sort_by(mut self, sort: ThreadSearchSort) -> Self {
        self.sort_by = Some(sort);
        self
    }

    /// Sets the sort direction.
    pub fn sort_order(mut self, order: SearchOrder) -> Self {
        self.sort_order = Some(order);
        self
    }

    /// Sets the number of threads, 1 to 25.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 25));
        self
    }

    /// Skips this number of results, 0 to 9975.
    pub fn offset(mut self, offset: u16) -> Self {
        self.offset = Some(offset.min(9975));
        self
    }
}

impl Endpoint for SearchThreads {
    type Response = ThreadSearchResponse;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("name", self.name.as_deref());
        query.push_opt("slop", self.slop);
        query.push_opt("min_id", self.min_id);
        query.push_opt("max_id", self.max_id);
        for tag in &self.tags {
            query.push("tag", tag);
        }
        query.push_opt(
            "tag_setting",
            self.tag_mode.map(ThreadSearchTagMode::as_str),
        );
        query.push_opt("archived", self.archived);
        query.push_opt("sort_by", self.sort_by.map(ThreadSearchSort::as_str));
        query.push_opt("sort_order", self.sort_order.map(SearchOrder::as_str));
        query.push_opt("limit", self.limit);
        query.push_opt("offset", self.offset);
        Route::new(
            Method::Get,
            "/channels/{channel_id}/threads/search",
            format!(
                "/channels/{}/threads/search{}",
                self.channel_id,
                query.finish()
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            if let Some(name) = &self.name {
                check_length("name", name, 0, 100)?;
            }
            check_count("tag", self.tags.len(), 20)
        })?;
        Ok(RequestBody::None)
    }
}

super::into_future!(
    CreateThreadFromMessage,
    CreateThread,
    CreateForumThread,
    JoinThread,
    LeaveThread,
    AddThreadMember,
    RemoveThreadMember,
    GetThreadMember,
    GetThreadMembers,
    GetPublicArchivedThreads,
    GetPrivateArchivedThreads,
    GetJoinedPrivateArchivedThreads,
    SearchThreads,
);

impl Client {
    /// Starts a thread from a message. The thread name has 1 to 100 characters.
    pub fn create_thread_from_message(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
        name: impl Into<String>,
    ) -> CreateThreadFromMessage {
        CreateThreadFromMessage {
            client: self.clone(),
            channel_id,
            message_id,
            fields: ThreadFields::new(name.into()),
            reason: None,
        }
    }

    /// Starts a thread that has no start message. The default type is a private thread.
    pub fn create_thread(&self, channel_id: ChannelId, name: impl Into<String>) -> CreateThread {
        CreateThread {
            client: self.clone(),
            channel_id,
            fields: ThreadFields::new(name.into()),
            reason: None,
        }
    }

    /// Starts a thread (a post) in a forum or media channel. Set the first message with the
    /// message setters.
    pub fn create_forum_thread(
        &self,
        channel_id: ChannelId,
        name: impl Into<String>,
    ) -> CreateForumThread {
        CreateForumThread {
            client: self.clone(),
            channel_id,
            fields: ThreadFields::new(name.into()),
            payload: MessagePayload::default(),
            sticker_ids: None,
            files: Vec::new(),
            reason: None,
        }
    }

    /// Adds the current user to a thread.
    pub fn join_thread(&self, channel_id: ChannelId) -> JoinThread {
        JoinThread {
            client: self.clone(),
            channel_id,
        }
    }

    /// Removes the current user from a thread.
    pub fn leave_thread(&self, channel_id: ChannelId) -> LeaveThread {
        LeaveThread {
            client: self.clone(),
            channel_id,
        }
    }

    /// Adds a member to a thread.
    pub fn add_thread_member(&self, channel_id: ChannelId, user_id: UserId) -> AddThreadMember {
        AddThreadMember {
            client: self.clone(),
            channel_id,
            user_id,
        }
    }

    /// Removes a member from a thread.
    pub fn remove_thread_member(
        &self,
        channel_id: ChannelId,
        user_id: UserId,
    ) -> RemoveThreadMember {
        RemoveThreadMember {
            client: self.clone(),
            channel_id,
            user_id,
        }
    }

    /// Gets one member of a thread.
    pub fn thread_member(&self, channel_id: ChannelId, user_id: UserId) -> GetThreadMember {
        GetThreadMember {
            client: self.clone(),
            channel_id,
            user_id,
            with_member: None,
        }
    }

    /// Gets the members of a thread.
    pub fn thread_members(&self, channel_id: ChannelId) -> GetThreadMembers {
        GetThreadMembers {
            client: self.clone(),
            channel_id,
            with_member: None,
            after: None,
            limit: None,
        }
    }

    /// Gets the archived public threads of a channel.
    pub fn public_archived_threads(&self, channel_id: ChannelId) -> GetPublicArchivedThreads {
        GetPublicArchivedThreads {
            client: self.clone(),
            channel_id,
            before: None,
            limit: None,
        }
    }

    /// Gets the archived private threads of a channel.
    pub fn private_archived_threads(&self, channel_id: ChannelId) -> GetPrivateArchivedThreads {
        GetPrivateArchivedThreads {
            client: self.clone(),
            channel_id,
            before: None,
            limit: None,
        }
    }

    /// Gets the archived private threads of a channel that the current user joined.
    pub fn joined_private_archived_threads(
        &self,
        channel_id: ChannelId,
    ) -> GetJoinedPrivateArchivedThreads {
        GetJoinedPrivateArchivedThreads {
            client: self.clone(),
            channel_id,
            before: None,
            limit: None,
        }
    }

    /// Searches the threads of a channel.
    pub fn search_threads(&self, channel_id: ChannelId) -> SearchThreads {
        SearchThreads {
            client: self.clone(),
            channel_id,
            name: None,
            slop: None,
            min_id: None,
            max_id: None,
            tags: Vec::new(),
            tag_mode: None,
            archived: None,
            sort_by: None,
            sort_order: None,
            limit: None,
            offset: None,
        }
    }
}
