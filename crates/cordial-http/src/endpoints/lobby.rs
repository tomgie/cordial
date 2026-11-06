//! Lobby endpoints (Discord Social SDK).
//!
//! Most lobby endpoints use the bot token. The endpoints below act for a user. They need a
//! client with an OAuth2 Bearer token ([`Token::bearer`](crate::Token::bearer)), mostly
//! with the `sdk.social_layer` scope:
//!
//! | Endpoint | Token |
//! |---|---|
//! | [`CreateOrJoinLobby`] | Bearer, `sdk.social_layer` |
//! | [`LeaveLobby`] | Bearer |
//! | [`EditLobbyChannelLink`] | Bearer, and the member has the `CAN_LINK_LOBBY` flag |
//! | [`CreateLobbyMessage`] | Bearer, `sdk.social_layer` |
//! | [`GetLobbyMessages`] | Bearer, `sdk.social_layer` |
//! | [`CreateLobbyInvite`] | Bearer, `sdk.social_layer` |
//!
//! All other lobby endpoints use the bot token.

use std::collections::HashMap;

use cordial_model::Patch;
use cordial_model::id::{ApplicationId, ChannelId, LobbyId, MessageId, UserId};
use cordial_model::lobby::{Lobby, LobbyMember, LobbyMemberFlags};
use cordial_model::message::{MessageFlags, MessageType};
use cordial_model::user::User;
use cordial_model::validate::ValidationError;
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route};

/// The maximum number of members in one lobby request.
pub const LOBBY_MEMBER_LIMIT: usize = 25;
/// The minimum idle timeout of a lobby, in seconds.
pub const IDLE_TIMEOUT_MIN: u32 = 5;
/// The maximum idle timeout of a lobby, in seconds (7 days).
pub const IDLE_TIMEOUT_MAX: u32 = 604_800;
/// The maximum total length of the keys and values of lobby metadata.
pub const METADATA_LIMIT: usize = 1000;
/// The maximum number of characters in the additional name of a lobby member.
pub const ADDITIONAL_NAME_LIMIT: usize = 80;
/// The maximum number of characters in a lobby secret.
pub const SECRET_LIMIT: usize = 250;
/// The maximum number of keys in moderation metadata.
pub const MODERATION_METADATA_KEY_COUNT_LIMIT: usize = 5;
/// The maximum number of characters in a moderation metadata key.
pub const MODERATION_METADATA_KEY_LIMIT: usize = 1024;
/// The maximum number of characters in a moderation metadata value.
pub const MODERATION_METADATA_VALUE_LIMIT: usize = 2000;

/// Key and value pairs of a lobby or a lobby member.
pub type Metadata = HashMap<String, String>;

/// A message in a lobby.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct LobbyMessage {
    /// The message ID.
    pub id: MessageId,
    /// The message type.
    #[serde(rename = "type")]
    pub kind: MessageType,
    /// The content.
    pub content: String,
    /// The lobby of the message.
    pub lobby_id: LobbyId,
    /// The same value as `lobby_id`, for compatibility with channel messages.
    pub channel_id: ChannelId,
    /// The user that sent the message.
    pub author: User,
    /// The lobby member data of the author when the message was sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lobby_member: Option<LobbyMessageMember>,
    /// Metadata that Discord sends to active clients only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Metadata>,
    /// Moderation metadata that the application set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moderation_metadata: Option<Metadata>,
    /// The message flags.
    #[serde(default)]
    pub flags: MessageFlags,
    /// The application that sent the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
}

/// The lobby member data of the author of a lobby message.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct LobbyMessageMember {
    /// The additional name of the author when the message was sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_name: Option<String>,
}

/// A single-use invite to the linked channel of a lobby. It expires after one hour.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct LobbyInvite {
    /// The invite code.
    pub code: String,
}

/// The fields of a lobby member that a request can set.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
struct MemberFields {
    #[serde(skip_serializing_if = "Patch::is_keep")]
    metadata: Patch<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<LobbyMemberFlags>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    additional_name: Patch<String>,
}

impl MemberFields {
    fn check(&self, field: &str) -> Result<(), ValidationError> {
        if let Patch::Set(metadata) = &self.metadata {
            check_metadata(&format!("{field}.metadata"), metadata)?;
        }
        if let Patch::Set(name) = &self.additional_name {
            let count = name.chars().count();
            if !(1..=ADDITIONAL_NAME_LIMIT).contains(&count) {
                return Err(ValidationError::new(
                    format!("{field}.additional_name"),
                    format!("has {count} characters; it must have 1 to {ADDITIONAL_NAME_LIMIT}"),
                ));
            }
        }
        Ok(())
    }
}

/// A member to add, update, or remove in a lobby request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[must_use]
pub struct LobbyMemberUpdate {
    id: UserId,
    #[serde(flatten)]
    fields: MemberFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    remove_member: Option<bool>,
}

impl LobbyMemberUpdate {
    /// Makes an update for a user. Fields that are not set keep their current value.
    pub fn new(user_id: UserId) -> Self {
        Self {
            id: user_id,
            fields: MemberFields::default(),
            remove_member: None,
        }
    }

    /// Sets the metadata of the member. It replaces the current metadata.
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.fields.metadata = Patch::Set(metadata);
        self
    }

    /// Removes the metadata of the member.
    pub fn clear_metadata(mut self) -> Self {
        self.fields.metadata = Patch::Clear;
        self
    }

    /// Sets the member flags.
    pub fn flags(mut self, flags: LobbyMemberFlags) -> Self {
        self.fields.flags = Some(flags);
        self
    }

    /// Sets an extra display name, 1 to 80 characters, for example a character name.
    pub fn additional_name(mut self, name: impl Into<String>) -> Self {
        self.fields.additional_name = Patch::Set(name.into());
        self
    }

    /// Removes the extra display name.
    pub fn clear_additional_name(mut self) -> Self {
        self.fields.additional_name = Patch::Clear;
        self
    }

    /// Removes the user from the lobby. Only [`BulkUpdateLobbyMembers`] accepts this field.
    pub fn remove(mut self) -> Self {
        self.remove_member = Some(true);
        self
    }

    /// Gives the user ID.
    #[must_use]
    pub fn user_id(&self) -> UserId {
        self.id
    }
}

fn check_metadata(field: &str, metadata: &Metadata) -> Result<(), ValidationError> {
    let total: usize = metadata
        .iter()
        .map(|(key, value)| key.chars().count() + value.chars().count())
        .sum();
    if total > METADATA_LIMIT {
        return Err(ValidationError::new(
            field,
            format!("has {total} characters in keys and values; the maximum is {METADATA_LIMIT}"),
        ));
    }
    Ok(())
}

fn check_idle_timeout(seconds: Option<u32>) -> Result<(), ValidationError> {
    match seconds {
        Some(seconds) if !(IDLE_TIMEOUT_MIN..=IDLE_TIMEOUT_MAX).contains(&seconds) => {
            Err(ValidationError::new(
                "idle_timeout_seconds",
                format!("is {seconds}; it must be {IDLE_TIMEOUT_MIN} to {IDLE_TIMEOUT_MAX}"),
            ))
        }
        _ => Ok(()),
    }
}

fn check_members(
    members: &[LobbyMemberUpdate],
    min: usize,
    allow_remove: bool,
) -> Result<(), ValidationError> {
    if members.len() < min || members.len() > LOBBY_MEMBER_LIMIT {
        return Err(ValidationError::new(
            "members",
            format!(
                "has {} members; it must have {min} to {LOBBY_MEMBER_LIMIT}",
                members.len()
            ),
        ));
    }
    for (index, member) in members.iter().enumerate() {
        let field = format!("members[{index}]");
        member.fields.check(&field)?;
        if !allow_remove && member.remove_member.is_some() {
            return Err(ValidationError::new(
                format!("{field}.remove_member"),
                "is accepted only by the bulk update endpoint",
            ));
        }
    }
    Ok(())
}

/// `POST /lobbies`: makes a lobby.
///
/// Discord Social SDK clients cannot join or leave a lobby that this endpoint makes.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateLobby {
    client: Client,
    body: CreateLobbyBody,
}

#[derive(Clone, Debug, Default, Serialize)]
struct CreateLobbyBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    members: Option<Vec<LobbyMemberUpdate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idle_timeout_seconds: Option<u32>,
}

impl CreateLobby {
    /// Sets the metadata of the lobby, up to 1000 characters in total.
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.body.metadata = Some(metadata);
        self
    }

    /// Adds members to the lobby (up to 25).
    pub fn members(mut self, members: impl IntoIterator<Item = LobbyMemberUpdate>) -> Self {
        self.body.members = Some(members.into_iter().collect());
        self
    }

    /// Sets the time in seconds before Discord closes an idle lobby, 5 to 604800.
    pub fn idle_timeout_seconds(mut self, seconds: u32) -> Self {
        self.body.idle_timeout_seconds = Some(seconds);
        self
    }
}

impl Endpoint for CreateLobby {
    type Response = Lobby;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/lobbies",
            "/lobbies".to_owned(),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            if let Some(metadata) = &self.body.metadata {
                check_metadata("metadata", metadata)?;
            }
            if let Some(members) = &self.body.members {
                check_members(members, 0, false)?;
            }
            check_idle_timeout(self.body.idle_timeout_seconds)
        })?;
        RequestBody::json(&self.body)
    }
}

/// `PUT /lobbies`: makes a lobby with a secret, or joins the current user to the lobby with
/// that secret.
///
/// This endpoint needs a Bearer token with the `sdk.social_layer` scope.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateOrJoinLobby {
    client: Client,
    body: CreateOrJoinLobbyBody,
}

#[derive(Clone, Debug, Serialize)]
struct CreateOrJoinLobbyBody {
    secret: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    idle_timeout_seconds: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lobby_metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    member_metadata: Option<Metadata>,
}

impl CreateOrJoinLobby {
    /// Sets the time in seconds before Discord closes an idle lobby, 5 to 604800.
    pub fn idle_timeout_seconds(mut self, seconds: u32) -> Self {
        self.body.idle_timeout_seconds = Some(seconds);
        self
    }

    /// Sets the metadata of the lobby. It replaces the current metadata.
    pub fn lobby_metadata(mut self, metadata: Metadata) -> Self {
        self.body.lobby_metadata = Some(metadata);
        self
    }

    /// Sets the metadata of the current user in the lobby.
    pub fn member_metadata(mut self, metadata: Metadata) -> Self {
        self.body.member_metadata = Some(metadata);
        self
    }
}

impl Endpoint for CreateOrJoinLobby {
    type Response = Lobby;

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/lobbies",
            "/lobbies".to_owned(),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            let count = self.body.secret.chars().count();
            if count == 0 || count > SECRET_LIMIT {
                return Err(ValidationError::new(
                    "secret",
                    format!("has {count} characters; it must have 1 to {SECRET_LIMIT}"),
                ));
            }
            if let Some(metadata) = &self.body.lobby_metadata {
                check_metadata("lobby_metadata", metadata)?;
            }
            if let Some(metadata) = &self.body.member_metadata {
                check_metadata("member_metadata", metadata)?;
            }
            check_idle_timeout(self.body.idle_timeout_seconds)
        })?;
        RequestBody::json(&self.body)
    }
}

/// `GET /lobbies/{lobby.id}`: a lobby.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetLobby {
    client: Client,
    lobby_id: LobbyId,
}

impl Endpoint for GetLobby {
    type Response = Lobby;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/lobbies/{lobby_id}",
            format!("/lobbies/{}", self.lobby_id),
            MajorParameter::None,
        )
    }
}

/// `PATCH /lobbies/{lobby.id}`: changes a lobby.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditLobby {
    client: Client,
    lobby_id: LobbyId,
    body: EditLobbyBody,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditLobbyBody {
    #[serde(skip_serializing_if = "Patch::is_keep")]
    metadata: Patch<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    members: Option<Vec<LobbyMemberUpdate>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idle_timeout_seconds: Option<u32>,
}

impl EditLobby {
    /// Sets the metadata of the lobby. It replaces the current metadata.
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.body.metadata = Patch::Set(metadata);
        self
    }

    /// Removes the metadata of the lobby.
    pub fn clear_metadata(mut self) -> Self {
        self.body.metadata = Patch::Clear;
        self
    }

    /// Replaces the members (up to 25). Discord removes members that are not in the list.
    pub fn members(mut self, members: impl IntoIterator<Item = LobbyMemberUpdate>) -> Self {
        self.body.members = Some(members.into_iter().collect());
        self
    }

    /// Sets the time in seconds before Discord closes an idle lobby, 5 to 604800.
    pub fn idle_timeout_seconds(mut self, seconds: u32) -> Self {
        self.body.idle_timeout_seconds = Some(seconds);
        self
    }
}

impl Endpoint for EditLobby {
    type Response = Lobby;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/lobbies/{lobby_id}",
            format!("/lobbies/{}", self.lobby_id),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            if let Patch::Set(metadata) = &self.body.metadata {
                check_metadata("metadata", metadata)?;
            }
            if let Some(members) = &self.body.members {
                check_members(members, 0, false)?;
            }
            check_idle_timeout(self.body.idle_timeout_seconds)
        })?;
        RequestBody::json(&self.body)
    }
}

/// `DELETE /lobbies/{lobby.id}`: deletes a lobby. It is safe to call for a deleted lobby.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteLobby {
    client: Client,
    lobby_id: LobbyId,
}

impl Endpoint for DeleteLobby {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/lobbies/{lobby_id}",
            format!("/lobbies/{}", self.lobby_id),
            MajorParameter::None,
        )
    }
}

/// `PUT /lobbies/{lobby.id}/members/{user.id}`: adds a user to a lobby, or updates the
/// member.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct AddLobbyMember {
    client: Client,
    lobby_id: LobbyId,
    user_id: UserId,
    fields: MemberFields,
}

impl AddLobbyMember {
    /// Sets the metadata of the member, up to 1000 characters in total.
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.fields.metadata = Patch::Set(metadata);
        self
    }

    /// Removes the metadata of the member.
    pub fn clear_metadata(mut self) -> Self {
        self.fields.metadata = Patch::Clear;
        self
    }

    /// Sets the member flags.
    pub fn flags(mut self, flags: LobbyMemberFlags) -> Self {
        self.fields.flags = Some(flags);
        self
    }

    /// Sets an extra display name, 1 to 80 characters, for example a character name.
    pub fn additional_name(mut self, name: impl Into<String>) -> Self {
        self.fields.additional_name = Patch::Set(name.into());
        self
    }

    /// Removes the extra display name.
    pub fn clear_additional_name(mut self) -> Self {
        self.fields.additional_name = Patch::Clear;
        self
    }
}

impl Endpoint for AddLobbyMember {
    type Response = LobbyMember;

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/lobbies/{lobby_id}/members/{user_id}",
            format!("/lobbies/{}/members/{}", self.lobby_id, self.user_id),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.fields.check("member"))?;
        RequestBody::json(&self.fields)
    }
}

/// `POST /lobbies/{lobby.id}/members/bulk`: adds, updates, or removes 1 to 25 members.
///
/// The response has the added and updated members. Discord drops users that fail a
/// permission check or that are in too many lobbies.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct BulkUpdateLobbyMembers {
    client: Client,
    lobby_id: LobbyId,
    members: Vec<LobbyMemberUpdate>,
}

impl Endpoint for BulkUpdateLobbyMembers {
    type Response = Vec<LobbyMember>;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/lobbies/{lobby_id}/members/bulk",
            format!("/lobbies/{}/members/bulk", self.lobby_id),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || check_members(&self.members, 1, true))?;
        RequestBody::json(&self.members)
    }
}

/// `DELETE /lobbies/{lobby.id}/members/{user.id}`: removes a user from a lobby.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct RemoveLobbyMember {
    client: Client,
    lobby_id: LobbyId,
    user_id: UserId,
}

impl Endpoint for RemoveLobbyMember {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/lobbies/{lobby_id}/members/{user_id}",
            format!("/lobbies/{}/members/{}", self.lobby_id, self.user_id),
            MajorParameter::None,
        )
    }
}

/// `DELETE /lobbies/{lobby.id}/members/@me`: the current user leaves a lobby.
///
/// This endpoint needs a Bearer token.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct LeaveLobby {
    client: Client,
    lobby_id: LobbyId,
}

impl Endpoint for LeaveLobby {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/lobbies/{lobby_id}/members/@me",
            format!("/lobbies/{}/members/@me", self.lobby_id),
            MajorParameter::None,
        )
    }
}

/// `PATCH /lobbies/{lobby.id}/channel-linking`: links a text channel to a lobby, or removes
/// the link.
///
/// This endpoint needs a Bearer token. The current user must be a lobby member with the
/// `CAN_LINK_LOBBY` flag.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditLobbyChannelLink {
    client: Client,
    lobby_id: LobbyId,
    channel_id: Option<ChannelId>,
}

#[derive(Serialize)]
struct EditLobbyChannelLinkBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<ChannelId>,
}

impl Endpoint for EditLobbyChannelLink {
    type Response = Lobby;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/lobbies/{lobby_id}/channel-linking",
            format!("/lobbies/{}/channel-linking", self.lobby_id),
            MajorParameter::None,
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        // Without a channel ID, Discord removes the link.
        RequestBody::json(&EditLobbyChannelLinkBody {
            channel_id: self.channel_id,
        })
    }
}

/// `POST /lobbies/{lobby.id}/messages`: sends a message to a lobby.
///
/// This endpoint needs a Bearer token with the `sdk.social_layer` scope. If the lobby has a
/// linked channel, Discord also sends the message to that channel.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateLobbyMessage {
    client: Client,
    lobby_id: LobbyId,
    body: CreateLobbyMessageBody,
}

#[derive(Clone, Debug, Serialize)]
struct CreateLobbyMessageBody {
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<MessageFlags>,
}

impl CreateLobbyMessage {
    /// Sets metadata that Discord sends with the message to active Social SDK clients.
    ///
    /// The message in a linked channel does not keep it.
    pub fn metadata(mut self, metadata: Metadata) -> Self {
        self.body.metadata = Some(metadata);
        self
    }

    /// Sets the message flags. Discord accepts only flags that the Social SDK can set.
    pub fn flags(mut self, flags: MessageFlags) -> Self {
        self.body.flags = Some(flags);
        self
    }
}

impl Endpoint for CreateLobbyMessage {
    type Response = LobbyMessage;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/lobbies/{lobby_id}/messages",
            format!("/lobbies/{}/messages", self.lobby_id),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            if self.body.content.is_empty() {
                return Err(ValidationError::new("content", "must not be empty"));
            }
            Ok(())
        })?;
        RequestBody::json(&self.body)
    }
}

/// `GET /lobbies/{lobby.id}/messages`: the newest messages of a lobby.
///
/// This endpoint needs a Bearer token with the `sdk.social_layer` scope.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetLobbyMessages {
    client: Client,
    lobby_id: LobbyId,
    limit: Option<u8>,
}

impl GetLobbyMessages {
    /// Sets the number of messages, 1 to 200. The default is 50.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 200));
        self
    }
}

impl Endpoint for GetLobbyMessages {
    type Response = Vec<LobbyMessage>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("limit", self.limit);
        Route::new(
            Method::Get,
            "/lobbies/{lobby_id}/messages",
            format!("/lobbies/{}/messages{}", self.lobby_id, query.finish()),
            MajorParameter::None,
        )
    }
}

/// Checks moderation metadata: up to 5 keys, keys up to 1024 and values up to 2000
/// characters.
pub(crate) fn check_moderation_metadata(metadata: &Metadata) -> Result<(), ValidationError> {
    if metadata.len() > MODERATION_METADATA_KEY_COUNT_LIMIT {
        return Err(ValidationError::new(
            "metadata",
            format!(
                "has {} keys; the maximum is {MODERATION_METADATA_KEY_COUNT_LIMIT}",
                metadata.len()
            ),
        ));
    }
    for (key, value) in metadata {
        if key.chars().count() > MODERATION_METADATA_KEY_LIMIT {
            return Err(ValidationError::new(
                "metadata",
                format!("has a key with more than {MODERATION_METADATA_KEY_LIMIT} characters"),
            ));
        }
        if value.chars().count() > MODERATION_METADATA_VALUE_LIMIT {
            return Err(ValidationError::new(
                format!("metadata.{key}"),
                format!("has more than {MODERATION_METADATA_VALUE_LIMIT} characters"),
            ));
        }
    }
    Ok(())
}

/// `PUT /lobbies/{lobby.id}/messages/{message.id}/moderation-metadata`: sets the moderation
/// metadata of a lobby message.
///
/// Discord sends the metadata to active Social SDK clients as a message update.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct UpdateLobbyMessageModerationMetadata {
    client: Client,
    lobby_id: LobbyId,
    message_id: MessageId,
    metadata: Metadata,
}

impl Endpoint for UpdateLobbyMessageModerationMetadata {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/lobbies/{lobby_id}/messages/{message_id}/moderation-metadata",
            format!(
                "/lobbies/{}/messages/{}/moderation-metadata",
                self.lobby_id, self.message_id
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || check_moderation_metadata(&self.metadata))?;
        RequestBody::json(&self.metadata)
    }
}

/// `POST /lobbies/{lobby.id}/members/@me/invites`: makes an invite to the linked channel of
/// a lobby for the current user.
///
/// This endpoint needs a Bearer token with the `sdk.social_layer` scope.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateLobbyInvite {
    client: Client,
    lobby_id: LobbyId,
}

impl Endpoint for CreateLobbyInvite {
    type Response = LobbyInvite;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/lobbies/{lobby_id}/members/@me/invites",
            format!("/lobbies/{}/members/@me/invites", self.lobby_id),
            MajorParameter::None,
        )
    }
}

/// `POST /lobbies/{lobby.id}/members/{user.id}/invites`: makes an invite to the linked
/// channel of a lobby for a user, for the application.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateLobbyInviteForUser {
    client: Client,
    lobby_id: LobbyId,
    user_id: UserId,
}

impl Endpoint for CreateLobbyInviteForUser {
    type Response = LobbyInvite;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/lobbies/{lobby_id}/members/{user_id}/invites",
            format!(
                "/lobbies/{}/members/{}/invites",
                self.lobby_id, self.user_id
            ),
            MajorParameter::None,
        )
    }
}

super::into_future!(
    CreateLobby,
    CreateOrJoinLobby,
    GetLobby,
    EditLobby,
    DeleteLobby,
    AddLobbyMember,
    BulkUpdateLobbyMembers,
    RemoveLobbyMember,
    LeaveLobby,
    EditLobbyChannelLink,
    CreateLobbyMessage,
    GetLobbyMessages,
    UpdateLobbyMessageModerationMetadata,
    CreateLobbyInvite,
    CreateLobbyInviteForUser,
);

impl Client {
    /// Makes a lobby.
    pub fn create_lobby(&self) -> CreateLobby {
        CreateLobby {
            client: self.clone(),
            body: CreateLobbyBody::default(),
        }
    }

    /// Makes a lobby with a secret, or joins it. Needs a Bearer token.
    pub fn create_or_join_lobby(&self, secret: impl Into<String>) -> CreateOrJoinLobby {
        CreateOrJoinLobby {
            client: self.clone(),
            body: CreateOrJoinLobbyBody {
                secret: secret.into(),
                idle_timeout_seconds: None,
                lobby_metadata: None,
                member_metadata: None,
            },
        }
    }

    /// Gets a lobby.
    pub fn lobby(&self, lobby_id: LobbyId) -> GetLobby {
        GetLobby {
            client: self.clone(),
            lobby_id,
        }
    }

    /// Changes a lobby.
    pub fn edit_lobby(&self, lobby_id: LobbyId) -> EditLobby {
        EditLobby {
            client: self.clone(),
            lobby_id,
            body: EditLobbyBody::default(),
        }
    }

    /// Deletes a lobby.
    pub fn delete_lobby(&self, lobby_id: LobbyId) -> DeleteLobby {
        DeleteLobby {
            client: self.clone(),
            lobby_id,
        }
    }

    /// Adds a user to a lobby, or updates the member.
    pub fn add_lobby_member(&self, lobby_id: LobbyId, user_id: UserId) -> AddLobbyMember {
        AddLobbyMember {
            client: self.clone(),
            lobby_id,
            user_id,
            fields: MemberFields::default(),
        }
    }

    /// Adds, updates, or removes 1 to 25 members of a lobby.
    pub fn bulk_update_lobby_members(
        &self,
        lobby_id: LobbyId,
        members: impl IntoIterator<Item = LobbyMemberUpdate>,
    ) -> BulkUpdateLobbyMembers {
        BulkUpdateLobbyMembers {
            client: self.clone(),
            lobby_id,
            members: members.into_iter().collect(),
        }
    }

    /// Removes a user from a lobby.
    pub fn remove_lobby_member(&self, lobby_id: LobbyId, user_id: UserId) -> RemoveLobbyMember {
        RemoveLobbyMember {
            client: self.clone(),
            lobby_id,
            user_id,
        }
    }

    /// Makes the current user leave a lobby. Needs a Bearer token.
    pub fn leave_lobby(&self, lobby_id: LobbyId) -> LeaveLobby {
        LeaveLobby {
            client: self.clone(),
            lobby_id,
        }
    }

    /// Links a text channel to a lobby. Needs a Bearer token.
    pub fn link_lobby_channel(
        &self,
        lobby_id: LobbyId,
        channel_id: ChannelId,
    ) -> EditLobbyChannelLink {
        EditLobbyChannelLink {
            client: self.clone(),
            lobby_id,
            channel_id: Some(channel_id),
        }
    }

    /// Removes the linked channel of a lobby. Needs a Bearer token.
    pub fn unlink_lobby_channel(&self, lobby_id: LobbyId) -> EditLobbyChannelLink {
        EditLobbyChannelLink {
            client: self.clone(),
            lobby_id,
            channel_id: None,
        }
    }

    /// Sends a message to a lobby. Needs a Bearer token.
    pub fn create_lobby_message(
        &self,
        lobby_id: LobbyId,
        content: impl Into<String>,
    ) -> CreateLobbyMessage {
        CreateLobbyMessage {
            client: self.clone(),
            lobby_id,
            body: CreateLobbyMessageBody {
                content: content.into(),
                metadata: None,
                flags: None,
            },
        }
    }

    /// Gets the newest messages of a lobby. Needs a Bearer token.
    pub fn lobby_messages(&self, lobby_id: LobbyId) -> GetLobbyMessages {
        GetLobbyMessages {
            client: self.clone(),
            lobby_id,
            limit: None,
        }
    }

    /// Sets the moderation metadata of a lobby message (up to 5 keys).
    pub fn update_lobby_message_moderation_metadata(
        &self,
        lobby_id: LobbyId,
        message_id: MessageId,
        metadata: Metadata,
    ) -> UpdateLobbyMessageModerationMetadata {
        UpdateLobbyMessageModerationMetadata {
            client: self.clone(),
            lobby_id,
            message_id,
            metadata,
        }
    }

    /// Makes an invite to the linked channel of a lobby for the current user. Needs a Bearer
    /// token.
    pub fn create_lobby_invite(&self, lobby_id: LobbyId) -> CreateLobbyInvite {
        CreateLobbyInvite {
            client: self.clone(),
            lobby_id,
        }
    }

    /// Makes an invite to the linked channel of a lobby for a user.
    pub fn create_lobby_invite_for_user(
        &self,
        lobby_id: LobbyId,
        user_id: UserId,
    ) -> CreateLobbyInviteForUser {
        CreateLobbyInviteForUser {
            client: self.clone(),
            lobby_id,
            user_id,
        }
    }
}
