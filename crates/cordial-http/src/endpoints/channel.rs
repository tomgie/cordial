//! Channel endpoints.

use std::fmt;

use cordial_model::Patch;
use cordial_model::channel::{
    AutoArchiveDuration, Channel, ChannelFlags, ChannelType, DefaultReaction, FollowedChannel,
    ForumLayout, ForumTag, SortOrder, VideoQualityMode,
};
use cordial_model::id::{ChannelId, ForumTagId, GenericId, RoleId, UserId};
use cordial_model::permissions::{PermissionOverwrite, PermissionOverwriteType, Permissions};
use cordial_model::validate::ValidationError;
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, ImageData, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route};

/// The maximum slowmode, in seconds.
const RATE_LIMIT_PER_USER_MAX: u32 = 21_600;

/// Checks that a text has `min` to `max` characters.
pub(super) fn check_length(
    field: &str,
    text: &str,
    min: usize,
    max: usize,
) -> Result<(), ValidationError> {
    let count = text.chars().count();
    if count < min || count > max {
        return Err(ValidationError::new(
            field,
            format!("has {count} characters; it must have {min} to {max}"),
        ));
    }
    Ok(())
}

/// Checks that a list has no more than `max` items.
pub(super) fn check_count(field: &str, count: usize, max: usize) -> Result<(), ValidationError> {
    if count > max {
        return Err(ValidationError::new(
            field,
            format!("has {count} items; the maximum is {max}"),
        ));
    }
    Ok(())
}

/// Checks that a slowmode value is in the range 0 to 21600 seconds.
pub(super) fn check_rate_limit(field: &str, seconds: Option<u32>) -> Result<(), ValidationError> {
    match seconds {
        Some(seconds) if seconds > RATE_LIMIT_PER_USER_MAX => Err(ValidationError::new(
            field,
            format!("is {seconds} seconds; the maximum is {RATE_LIMIT_PER_USER_MAX}"),
        )),
        _ => Ok(()),
    }
}

/// The computed permissions of a member or a role in a channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ComputedPermissions {
    /// The permissions.
    pub permissions: Permissions,
}

/// `GET /channels/{channel.id}`: a channel or a thread.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetChannel {
    client: Client,
    channel_id: ChannelId,
}

impl Endpoint for GetChannel {
    type Response = Channel;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/channels/{channel_id}",
            format!("/channels/{}", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `PATCH /channels/{channel.id}`: edits a guild channel, a group DM, or a thread.
///
/// Each field applies only to some channel types. Discord answers with an error for a field
/// that does not apply.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditChannel {
    client: Client,
    channel_id: ChannelId,
    fields: EditChannelFields,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditChannelFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<ImageData>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<ChannelType>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    position: Patch<i32>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    topic: Patch<String>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    nsfw: Patch<bool>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    rate_limit_per_user: Patch<u32>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    bitrate: Patch<u32>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    user_limit: Patch<u32>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    permission_overwrites: Patch<Vec<PermissionOverwrite>>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    parent_id: Patch<ChannelId>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    rtc_region: Patch<String>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    video_quality_mode: Patch<VideoQualityMode>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    default_auto_archive_duration: Patch<AutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<ChannelFlags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    available_tags: Option<Vec<ForumTag>>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    default_reaction_emoji: Patch<DefaultReaction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_thread_rate_limit_per_user: Option<u32>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    default_sort_order: Patch<SortOrder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_forum_layout: Option<ForumLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_archive_duration: Option<AutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    locked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invitable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    applied_tags: Option<Vec<ForumTagId>>,
}

impl EditChannel {
    /// Sets the name, 1 to 100 characters. All channel types.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.fields.name = Some(name.into());
        self
    }

    /// Sets the icon of a group DM.
    pub fn icon(mut self, icon: ImageData) -> Self {
        self.fields.icon = Some(icon);
        self
    }

    /// Converts a text channel to an announcement channel, or the opposite. The guild must
    /// have the `NEWS` feature.
    pub fn kind(mut self, kind: ChannelType) -> Self {
        self.fields.kind = Some(kind);
        self
    }

    /// Sets the sort position.
    pub fn position(mut self, position: i32) -> Self {
        self.fields.position = Patch::Set(position);
        self
    }

    /// Sets the topic: up to 4096 characters for forum and media channels, else up to 1024.
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.fields.topic = Patch::Set(topic.into());
        self
    }

    /// Removes the topic.
    pub fn clear_topic(mut self) -> Self {
        self.fields.topic = Patch::Clear;
        self
    }

    /// Sets whether the channel is age-restricted.
    pub fn nsfw(mut self, nsfw: bool) -> Self {
        self.fields.nsfw = Patch::Set(nsfw);
        self
    }

    /// Sets the slowmode in seconds (0 to 21600). Also applies to threads.
    pub fn rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.fields.rate_limit_per_user = Patch::Set(seconds);
        self
    }

    /// Sets the bitrate of a voice or stage channel, in bits per second (minimum 8000).
    pub fn bitrate(mut self, bitrate: u32) -> Self {
        self.fields.bitrate = Patch::Set(bitrate);
        self
    }

    /// Sets the user limit of a voice or stage channel. 0 means no limit.
    pub fn user_limit(mut self, user_limit: u32) -> Self {
        self.fields.user_limit = Patch::Set(user_limit);
        self
    }

    /// Replaces all permission overwrites. This needs the `MANAGE_ROLES` permission.
    pub fn permission_overwrites(mut self, overwrites: Vec<PermissionOverwrite>) -> Self {
        self.fields.permission_overwrites = Patch::Set(overwrites);
        self
    }

    /// Moves the channel into a category.
    pub fn parent(mut self, parent_id: ChannelId) -> Self {
        self.fields.parent_id = Patch::Set(parent_id);
        self
    }

    /// Moves the channel out of its category.
    pub fn clear_parent(mut self) -> Self {
        self.fields.parent_id = Patch::Clear;
        self
    }

    /// Sets the voice region of a voice or stage channel.
    pub fn rtc_region(mut self, region: impl Into<String>) -> Self {
        self.fields.rtc_region = Patch::Set(region.into());
        self
    }

    /// Lets Discord choose the voice region automatically.
    pub fn clear_rtc_region(mut self) -> Self {
        self.fields.rtc_region = Patch::Clear;
        self
    }

    /// Sets the camera video quality of a voice or stage channel.
    pub fn video_quality_mode(mut self, mode: VideoQualityMode) -> Self {
        self.fields.video_quality_mode = Patch::Set(mode);
        self
    }

    /// Sets the default auto-archive duration that clients use for new threads.
    pub fn default_auto_archive_duration(mut self, duration: AutoArchiveDuration) -> Self {
        self.fields.default_auto_archive_duration = Patch::Set(duration);
        self
    }

    /// Sets the channel flags. For a thread, `PINNED` applies only in forum and media
    /// channels.
    pub fn flags(mut self, flags: ChannelFlags) -> Self {
        self.fields.flags = Some(flags);
        self
    }

    /// Sets the tags that threads in a forum or media channel can have (up to 20).
    pub fn available_tags(mut self, tags: Vec<ForumTag>) -> Self {
        self.fields.available_tags = Some(tags);
        self
    }

    /// Sets the emoji of the reaction button on posts in a forum or media channel.
    pub fn default_reaction_emoji(mut self, emoji: DefaultReaction) -> Self {
        self.fields.default_reaction_emoji = Patch::Set(emoji);
        self
    }

    /// Removes the emoji of the reaction button on posts.
    pub fn clear_default_reaction_emoji(mut self) -> Self {
        self.fields.default_reaction_emoji = Patch::Clear;
        self
    }

    /// Sets the initial slowmode of new threads, in seconds.
    pub fn default_thread_rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.fields.default_thread_rate_limit_per_user = Some(seconds);
        self
    }

    /// Sets the default sort order of posts in a forum or media channel.
    pub fn default_sort_order(mut self, order: SortOrder) -> Self {
        self.fields.default_sort_order = Patch::Set(order);
        self
    }

    /// Removes the default sort order of posts.
    pub fn clear_default_sort_order(mut self) -> Self {
        self.fields.default_sort_order = Patch::Clear;
        self
    }

    /// Sets the default layout of a forum channel.
    pub fn default_forum_layout(mut self, layout: ForumLayout) -> Self {
        self.fields.default_forum_layout = Some(layout);
        self
    }

    /// Archives or unarchives a thread.
    pub fn archived(mut self, archived: bool) -> Self {
        self.fields.archived = Some(archived);
        self
    }

    /// Sets the time of inactivity after which a thread is archived.
    pub fn auto_archive_duration(mut self, duration: AutoArchiveDuration) -> Self {
        self.fields.auto_archive_duration = Some(duration);
        self
    }

    /// Locks or unlocks a thread. Only members with `MANAGE_THREADS` can unarchive a locked
    /// thread.
    pub fn locked(mut self, locked: bool) -> Self {
        self.fields.locked = Some(locked);
        self
    }

    /// Sets whether members that are not moderators can add others to a private thread.
    pub fn invitable(mut self, invitable: bool) -> Self {
        self.fields.invitable = Some(invitable);
        self
    }

    /// Sets the tags of a thread in a forum or media channel (up to 5).
    pub fn applied_tags(mut self, tags: Vec<ForumTagId>) -> Self {
        self.fields.applied_tags = Some(tags);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    fn validate(&self) -> Result<(), ValidationError> {
        let fields = &self.fields;
        if let Some(name) = &fields.name {
            check_length("name", name, 1, 100)?;
        }
        if let Patch::Set(topic) = &fields.topic {
            check_length("topic", topic, 0, 4096)?;
        }
        if let Patch::Set(seconds) = fields.rate_limit_per_user {
            check_rate_limit("rate_limit_per_user", Some(seconds))?;
        }
        check_rate_limit(
            "default_thread_rate_limit_per_user",
            fields.default_thread_rate_limit_per_user,
        )?;
        if let Patch::Set(bitrate) = fields.bitrate
            && bitrate < 8000
        {
            return Err(ValidationError::new(
                "bitrate",
                format!("is {bitrate}; the minimum is 8000"),
            ));
        }
        if let Patch::Set(user_limit) = fields.user_limit
            && user_limit > 10_000
        {
            return Err(ValidationError::new(
                "user_limit",
                format!("is {user_limit}; the maximum is 10000"),
            ));
        }
        if let Some(tags) = &fields.available_tags {
            check_count("available_tags", tags.len(), 20)?;
        }
        if let Some(tags) = &fields.applied_tags {
            check_count("applied_tags", tags.len(), 5)?;
        }
        Ok(())
    }
}

impl Endpoint for EditChannel {
    type Response = Channel;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/channels/{channel_id}",
            format!("/channels/{}", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.validate())?;
        RequestBody::json(&self.fields)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /channels/{channel.id}`: deletes a guild channel or a thread, or closes a DM.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteChannel {
    client: Client,
    channel_id: ChannelId,
    reason: Option<String>,
}

impl DeleteChannel {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for DeleteChannel {
    type Response = Channel;

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/channels/{channel_id}",
            format!("/channels/{}", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `PUT /channels/{channel.id}/permissions/{overwrite.id}`: sets the permission overwrite of
/// a role or a member.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditChannelPermissions {
    client: Client,
    channel_id: ChannelId,
    overwrite_id: GenericId,
    body: EditChannelPermissionsBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct EditChannelPermissionsBody {
    allow: Permissions,
    deny: Permissions,
    #[serde(rename = "type")]
    kind: PermissionOverwriteType,
}

impl EditChannelPermissions {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditChannelPermissions {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/channels/{channel_id}/permissions/{overwrite_id}",
            format!(
                "/channels/{}/permissions/{}",
                self.channel_id, self.overwrite_id
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /channels/{channel.id}/permissions/{overwrite.id}`: removes the permission
/// overwrite of a role or a member.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteChannelPermission {
    client: Client,
    channel_id: ChannelId,
    overwrite_id: GenericId,
    reason: Option<String>,
}

impl DeleteChannelPermission {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for DeleteChannelPermission {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/channels/{channel_id}/permissions/{overwrite_id}",
            format!(
                "/channels/{}/permissions/{}",
                self.channel_id, self.overwrite_id
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /channels/{channel.id}/permissions/member/{user.id}`: the computed permissions of a
/// member in a channel.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetChannelMemberPermissions {
    client: Client,
    channel_id: ChannelId,
    user_id: UserId,
    implicit: Option<bool>,
}

impl GetChannelMemberPermissions {
    /// Sets whether Discord applies the implicit permission rules. The default is `true`.
    /// Discord does not apply them to bots.
    pub fn implicit(mut self, implicit: bool) -> Self {
        self.implicit = Some(implicit);
        self
    }
}

impl Endpoint for GetChannelMemberPermissions {
    type Response = ComputedPermissions;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("implicit", self.implicit);
        Route::new(
            Method::Get,
            "/channels/{channel_id}/permissions/member/{user_id}",
            format!(
                "/channels/{}/permissions/member/{}{}",
                self.channel_id,
                self.user_id,
                query.finish()
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `GET /channels/{channel.id}/permissions/role/{role.id}`: the permissions of a role and
/// `@everyone` in a channel, with their overwrites.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetChannelRolePermissions {
    client: Client,
    channel_id: ChannelId,
    role_id: RoleId,
    implicit: Option<bool>,
}

impl GetChannelRolePermissions {
    /// Sets whether Discord applies the implicit permission rules. The default is `true`.
    pub fn implicit(mut self, implicit: bool) -> Self {
        self.implicit = Some(implicit);
        self
    }
}

impl Endpoint for GetChannelRolePermissions {
    type Response = ComputedPermissions;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("implicit", self.implicit);
        Route::new(
            Method::Get,
            "/channels/{channel_id}/permissions/role/{role_id}",
            format!(
                "/channels/{}/permissions/role/{}{}",
                self.channel_id,
                self.role_id,
                query.finish()
            ),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

/// `PUT /channels/{channel.id}/voice-status`: sets or removes the status of a voice channel.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct SetVoiceChannelStatus {
    client: Client,
    channel_id: ChannelId,
    body: VoiceStatusBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct VoiceStatusBody {
    status: Option<String>,
}

impl SetVoiceChannelStatus {
    /// Sets the status text (up to 500 characters). Without it, the request removes the
    /// status.
    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.body.status = Some(status.into());
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for SetVoiceChannelStatus {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/channels/{channel_id}/voice-status",
            format!("/channels/{}/voice-status", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || match &self.body.status {
            Some(status) => check_length("status", status, 0, 500),
            None => Ok(()),
        })?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `POST /channels/{channel.id}/followers`: follows an announcement channel into a target
/// channel.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct FollowAnnouncementChannel {
    client: Client,
    channel_id: ChannelId,
    body: FollowBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct FollowBody {
    webhook_channel_id: ChannelId,
}

impl FollowAnnouncementChannel {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for FollowAnnouncementChannel {
    type Response = FollowedChannel;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/channels/{channel_id}/followers",
            format!("/channels/{}/followers", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `POST /channels/{channel.id}/typing`: shows the typing indicator for 10 seconds.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct TriggerTyping {
    client: Client,
    channel_id: ChannelId,
}

impl Endpoint for TriggerTyping {
    // Discord answers with 204, or with 200 and an empty object.
    type Response = serde::de::IgnoredAny;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/channels/{channel_id}/typing",
            format!("/channels/{}/typing", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

impl std::future::IntoFuture for TriggerTyping {
    type Output = Result<(), Error>;
    type IntoFuture = crate::transport::BoxFuture<'static, Self::Output>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.client.request(&self).await.map(|_| ()) })
    }
}

/// `PUT /channels/{channel.id}/recipients/{user.id}`: adds a user to a group DM.
///
/// The response is the channel when Discord answers with 201, else `None`.
#[derive(Clone)]
#[must_use = "a request does nothing until it is awaited"]
pub struct AddGroupDmRecipient {
    client: Client,
    channel_id: ChannelId,
    user_id: UserId,
    body: AddRecipientBody,
}

#[derive(Clone, Serialize)]
struct AddRecipientBody {
    access_token: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    nick: Option<String>,
}

impl AddGroupDmRecipient {
    /// Sets the nickname of the user in the group DM.
    pub fn nick(mut self, nick: impl Into<String>) -> Self {
        self.body.nick = Some(nick.into());
        self
    }
}

impl fmt::Debug for AddGroupDmRecipient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AddGroupDmRecipient")
            .field("channel_id", &self.channel_id)
            .field("user_id", &self.user_id)
            .field("access_token", &"<redacted>")
            .field("nick", &self.body.nick)
            .finish_non_exhaustive()
    }
}

impl Endpoint for AddGroupDmRecipient {
    type Response = Option<Channel>;

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/channels/{channel_id}/recipients/{user_id}",
            format!("/channels/{}/recipients/{}", self.channel_id, self.user_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }
}

/// `DELETE /channels/{channel.id}/recipients/{user.id}`: removes a user from a group DM.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct RemoveGroupDmRecipient {
    client: Client,
    channel_id: ChannelId,
    user_id: UserId,
}

impl Endpoint for RemoveGroupDmRecipient {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/channels/{channel_id}/recipients/{user_id}",
            format!("/channels/{}/recipients/{}", self.channel_id, self.user_id),
            MajorParameter::Channel(self.channel_id),
        )
    }
}

super::into_future!(
    GetChannel,
    EditChannel,
    DeleteChannel,
    EditChannelPermissions,
    DeleteChannelPermission,
    GetChannelMemberPermissions,
    GetChannelRolePermissions,
    SetVoiceChannelStatus,
    FollowAnnouncementChannel,
    AddGroupDmRecipient,
    RemoveGroupDmRecipient,
);

impl Client {
    /// Gets a channel or a thread.
    pub fn channel(&self, channel_id: ChannelId) -> GetChannel {
        GetChannel {
            client: self.clone(),
            channel_id,
        }
    }

    /// Edits a guild channel, a group DM, or a thread.
    pub fn edit_channel(&self, channel_id: ChannelId) -> EditChannel {
        EditChannel {
            client: self.clone(),
            channel_id,
            fields: EditChannelFields::default(),
            reason: None,
        }
    }

    /// Deletes a guild channel or a thread, or closes a DM. A deleted guild channel cannot
    /// come back.
    pub fn delete_channel(&self, channel_id: ChannelId) -> DeleteChannel {
        DeleteChannel {
            client: self.clone(),
            channel_id,
            reason: None,
        }
    }

    /// Sets the permission overwrite of a role or a member in a guild channel.
    pub fn edit_channel_permissions(
        &self,
        channel_id: ChannelId,
        overwrite: &PermissionOverwrite,
    ) -> EditChannelPermissions {
        EditChannelPermissions {
            client: self.clone(),
            channel_id,
            overwrite_id: overwrite.id,
            body: EditChannelPermissionsBody {
                allow: overwrite.allow,
                deny: overwrite.deny,
                kind: overwrite.kind,
            },
            reason: None,
        }
    }

    /// Removes the permission overwrite of a role or a member. Give the role ID or the user
    /// ID, for example `role_id.cast()`.
    pub fn delete_channel_permission(
        &self,
        channel_id: ChannelId,
        overwrite_id: GenericId,
    ) -> DeleteChannelPermission {
        DeleteChannelPermission {
            client: self.clone(),
            channel_id,
            overwrite_id,
            reason: None,
        }
    }

    /// Gets the computed permissions of a member in a channel.
    pub fn channel_member_permissions(
        &self,
        channel_id: ChannelId,
        user_id: UserId,
    ) -> GetChannelMemberPermissions {
        GetChannelMemberPermissions {
            client: self.clone(),
            channel_id,
            user_id,
            implicit: None,
        }
    }

    /// Gets the permissions of a role and `@everyone` in a channel.
    pub fn channel_role_permissions(
        &self,
        channel_id: ChannelId,
        role_id: RoleId,
    ) -> GetChannelRolePermissions {
        GetChannelRolePermissions {
            client: self.clone(),
            channel_id,
            role_id,
            implicit: None,
        }
    }

    /// Sets the status of a voice channel. Without a status text, the request removes the
    /// status.
    pub fn set_voice_channel_status(&self, channel_id: ChannelId) -> SetVoiceChannelStatus {
        SetVoiceChannelStatus {
            client: self.clone(),
            channel_id,
            body: VoiceStatusBody { status: None },
            reason: None,
        }
    }

    /// Follows an announcement channel. Discord sends its messages to the target channel.
    pub fn follow_announcement_channel(
        &self,
        channel_id: ChannelId,
        webhook_channel_id: ChannelId,
    ) -> FollowAnnouncementChannel {
        FollowAnnouncementChannel {
            client: self.clone(),
            channel_id,
            body: FollowBody { webhook_channel_id },
            reason: None,
        }
    }

    /// Shows the typing indicator in a channel for 10 seconds.
    pub fn trigger_typing(&self, channel_id: ChannelId) -> TriggerTyping {
        TriggerTyping {
            client: self.clone(),
            channel_id,
        }
    }

    /// Adds a user to a group DM. The access token must have the `gdm.join` scope.
    pub fn add_group_dm_recipient(
        &self,
        channel_id: ChannelId,
        user_id: UserId,
        access_token: impl Into<String>,
    ) -> AddGroupDmRecipient {
        AddGroupDmRecipient {
            client: self.clone(),
            channel_id,
            user_id,
            body: AddRecipientBody {
                access_token: access_token.into(),
                nick: None,
            },
        }
    }

    /// Removes a user from a group DM.
    pub fn remove_group_dm_recipient(
        &self,
        channel_id: ChannelId,
        user_id: UserId,
    ) -> RemoveGroupDmRecipient {
        RemoveGroupDmRecipient {
            client: self.clone(),
            channel_id,
            user_id,
        }
    }
}
