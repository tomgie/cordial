//! Guild member endpoints.

use std::fmt;

use cordial_model::guild::{Member, MemberFlags};
use cordial_model::id::{ChannelId, GuildId, RoleId, UserId};
use cordial_model::{Patch, Timestamp};
use serde::Serialize;

use super::guild::ComputedPermissions;
use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, ImageData, RequestBody};
use crate::route::{MajorParameter, Method, Query, Route};

/// `GET /guilds/{guild.id}/members/{user.id}`: a member of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildMember {
    client: Client,
    guild_id: GuildId,
    user_id: UserId,
}

impl Endpoint for GetGuildMember {
    type Response = Member;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/members/{user_id}",
            format!("/guilds/{}/members/{}", self.guild_id, self.user_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/members`: members of a guild, by user ID. Needs the
/// `GUILD_MEMBERS` intent.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildMembers {
    client: Client,
    guild_id: GuildId,
    limit: Option<u16>,
    after: Option<UserId>,
}

impl GetGuildMembers {
    /// Sets the number of members, 1 to 1000. The default is 1.
    pub fn limit(mut self, limit: u16) -> Self {
        self.limit = Some(limit.clamp(1, 1000));
        self
    }

    /// Gets members with a user ID greater than this one. Use the highest ID of the last page.
    pub fn after(mut self, user_id: UserId) -> Self {
        self.after = Some(user_id);
        self
    }
}

impl Endpoint for GetGuildMembers {
    type Response = Vec<Member>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("limit", self.limit);
        query.push_opt("after", self.after);
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/members",
            format!("/guilds/{}/members{}", self.guild_id, query.finish()),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/members/search`: members whose username or nickname starts with a
/// text.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct SearchGuildMembers {
    client: Client,
    guild_id: GuildId,
    query: String,
    limit: Option<u16>,
}

impl SearchGuildMembers {
    /// Sets the number of members, 1 to 1000. The default is 1.
    pub fn limit(mut self, limit: u16) -> Self {
        self.limit = Some(limit.clamp(1, 1000));
        self
    }
}

impl Endpoint for SearchGuildMembers {
    type Response = Vec<Member>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push("query", &self.query);
        query.push_opt("limit", self.limit);
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/members/search",
            format!("/guilds/{}/members/search{}", self.guild_id, query.finish()),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/permissions/member/{user.id}`: the guild permissions of a member,
/// from the roles of the member and `@everyone`.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildMemberPermissions {
    client: Client,
    guild_id: GuildId,
    user_id: UserId,
}

impl Endpoint for GetGuildMemberPermissions {
    type Response = ComputedPermissions;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/permissions/member/{user_id}",
            format!(
                "/guilds/{}/permissions/member/{}",
                self.guild_id, self.user_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// An OAuth2 access token. `Debug` does not show it.
#[derive(Clone, Serialize)]
#[serde(transparent)]
struct AccessToken(String);

impl fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AccessToken(<hidden>)")
    }
}

/// `PUT /guilds/{guild.id}/members/{user.id}`: adds a user to a guild with an OAuth2 access
/// token of the user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct AddGuildMember {
    client: Client,
    guild_id: GuildId,
    user_id: UserId,
    body: AddGuildMemberBody,
}

#[derive(Clone, Debug, Serialize)]
struct AddGuildMemberBody {
    access_token: AccessToken,
    #[serde(skip_serializing_if = "Option::is_none")]
    nick: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    roles: Option<Vec<RoleId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mute: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deaf: Option<bool>,
}

impl AddGuildMember {
    /// Sets the nickname. Needs `MANAGE_NICKNAMES`.
    pub fn nick(mut self, nick: impl Into<String>) -> Self {
        self.body.nick = Some(nick.into());
        self
    }

    /// Sets the roles. Needs `MANAGE_ROLES`.
    pub fn roles(mut self, role_ids: Vec<RoleId>) -> Self {
        self.body.roles = Some(role_ids);
        self
    }

    /// Mutes the user in voice channels. Needs `MUTE_MEMBERS`.
    pub fn mute(mut self, mute: bool) -> Self {
        self.body.mute = Some(mute);
        self
    }

    /// Deafens the user in voice channels. Needs `DEAFEN_MEMBERS`.
    pub fn deaf(mut self, deaf: bool) -> Self {
        self.body.deaf = Some(deaf);
        self
    }
}

impl Endpoint for AddGuildMember {
    /// The new member, or `None` when the user is already a member (status 204).
    type Response = Option<Member>;

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/guilds/{guild_id}/members/{user_id}",
            format!("/guilds/{}/members/{}", self.guild_id, self.user_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }
}

/// `PATCH /guilds/{guild.id}/members/{user.id}`: changes a member.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildMember {
    client: Client,
    guild_id: GuildId,
    user_id: UserId,
    body: EditGuildMemberBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditGuildMemberBody {
    #[serde(skip_serializing_if = "Patch::is_keep")]
    nick: Patch<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    roles: Option<Vec<RoleId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mute: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deaf: Option<bool>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    channel_id: Patch<ChannelId>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    communication_disabled_until: Patch<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<MemberFlags>,
}

impl EditGuildMember {
    /// Sets the nickname. Needs `MANAGE_NICKNAMES`.
    pub fn nick(mut self, nick: impl Into<String>) -> Self {
        self.body.nick = Patch::Set(nick.into());
        self
    }

    /// Removes the nickname. Needs `MANAGE_NICKNAMES`.
    pub fn clear_nick(mut self) -> Self {
        self.body.nick = Patch::Clear;
        self
    }

    /// Sets all roles of the member. Needs `MANAGE_ROLES`.
    pub fn roles(mut self, role_ids: Vec<RoleId>) -> Self {
        self.body.roles = Some(role_ids);
        self
    }

    /// Mutes the member in voice channels. Needs `MUTE_MEMBERS`. The member must be in a
    /// voice channel.
    pub fn mute(mut self, mute: bool) -> Self {
        self.body.mute = Some(mute);
        self
    }

    /// Deafens the member in voice channels. Needs `DEAFEN_MEMBERS`. The member must be in a
    /// voice channel.
    pub fn deaf(mut self, deaf: bool) -> Self {
        self.body.deaf = Some(deaf);
        self
    }

    /// Moves the member to a voice channel. Needs `MOVE_MEMBERS` and `CONNECT` there.
    pub fn channel_id(mut self, channel_id: ChannelId) -> Self {
        self.body.channel_id = Patch::Set(channel_id);
        self
    }

    /// Disconnects the member from voice. Needs `MOVE_MEMBERS`.
    pub fn disconnect(mut self) -> Self {
        self.body.channel_id = Patch::Clear;
        self
    }

    /// Times out the member until a time, at most 28 days in the future. Needs
    /// `MODERATE_MEMBERS`.
    pub fn communication_disabled_until(mut self, until: Timestamp) -> Self {
        self.body.communication_disabled_until = Patch::Set(until);
        self
    }

    /// Ends the timeout of the member. Needs `MODERATE_MEMBERS`.
    pub fn clear_communication_disabled_until(mut self) -> Self {
        self.body.communication_disabled_until = Patch::Clear;
        self
    }

    /// Sets the member flags. Only `BYPASSES_VERIFICATION` can change.
    pub fn flags(mut self, flags: MemberFlags) -> Self {
        self.body.flags = Some(flags);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditGuildMember {
    type Response = Member;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/members/{user_id}",
            format!("/guilds/{}/members/{}", self.guild_id, self.user_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `PATCH /guilds/{guild.id}/members/@me`: changes the current member.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditCurrentMember {
    client: Client,
    guild_id: GuildId,
    body: EditCurrentMemberBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditCurrentMemberBody {
    #[serde(skip_serializing_if = "Patch::is_keep")]
    nick: Patch<String>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    banner: Patch<ImageData>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    avatar: Patch<ImageData>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    bio: Patch<String>,
}

impl EditCurrentMember {
    /// Sets the nickname. Needs `CHANGE_NICKNAME`.
    pub fn nick(mut self, nick: impl Into<String>) -> Self {
        self.body.nick = Patch::Set(nick.into());
        self
    }

    /// Removes the nickname.
    pub fn clear_nick(mut self) -> Self {
        self.body.nick = Patch::Clear;
        self
    }

    /// Sets the banner in the guild.
    pub fn banner(mut self, banner: ImageData) -> Self {
        self.body.banner = Patch::Set(banner);
        self
    }

    /// Removes the banner in the guild.
    pub fn clear_banner(mut self) -> Self {
        self.body.banner = Patch::Clear;
        self
    }

    /// Sets the avatar in the guild.
    pub fn avatar(mut self, avatar: ImageData) -> Self {
        self.body.avatar = Patch::Set(avatar);
        self
    }

    /// Removes the avatar in the guild.
    pub fn clear_avatar(mut self) -> Self {
        self.body.avatar = Patch::Clear;
        self
    }

    /// Sets the bio in the guild.
    pub fn bio(mut self, bio: impl Into<String>) -> Self {
        self.body.bio = Patch::Set(bio.into());
        self
    }

    /// Removes the bio in the guild.
    pub fn clear_bio(mut self) -> Self {
        self.body.bio = Patch::Clear;
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditCurrentMember {
    type Response = Member;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/members/@me",
            format!("/guilds/{}/members/@me", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `PATCH /guilds/{guild.id}/members/@me/nick`: changes the nickname of the current user.
///
/// Deprecated by Discord: use [`Client::edit_current_member`].
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditCurrentUserNick {
    client: Client,
    guild_id: GuildId,
    body: EditCurrentUserNickBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditCurrentUserNickBody {
    #[serde(skip_serializing_if = "Patch::is_keep")]
    nick: Patch<String>,
}

impl EditCurrentUserNick {
    /// Sets the nickname. Needs `CHANGE_NICKNAME`.
    pub fn nick(mut self, nick: impl Into<String>) -> Self {
        self.body.nick = Patch::Set(nick.into());
        self
    }

    /// Removes the nickname.
    pub fn clear_nick(mut self) -> Self {
        self.body.nick = Patch::Clear;
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditCurrentUserNick {
    /// The docs say only "the nickname". The shape is not documented, so the response stays
    /// as raw JSON.
    type Response = serde_json::Value;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/members/@me/nick",
            format!("/guilds/{}/members/@me/nick", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `PUT /guilds/{guild.id}/members/{user.id}/roles/{role.id}`: gives a role to a member.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct AddGuildMemberRole {
    client: Client,
    guild_id: GuildId,
    user_id: UserId,
    role_id: RoleId,
    reason: Option<String>,
}

impl AddGuildMemberRole {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for AddGuildMemberRole {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/guilds/{guild_id}/members/{user_id}/roles/{role_id}",
            format!(
                "/guilds/{}/members/{}/roles/{}",
                self.guild_id, self.user_id, self.role_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /guilds/{guild.id}/members/{user.id}/roles/{role.id}`: takes a role from a member.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct RemoveGuildMemberRole {
    client: Client,
    guild_id: GuildId,
    user_id: UserId,
    role_id: RoleId,
    reason: Option<String>,
}

impl RemoveGuildMemberRole {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for RemoveGuildMemberRole {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/guilds/{guild_id}/members/{user_id}/roles/{role_id}",
            format!(
                "/guilds/{}/members/{}/roles/{}",
                self.guild_id, self.user_id, self.role_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /guilds/{guild.id}/members/{user.id}`: removes (kicks) a member from a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct RemoveGuildMember {
    client: Client,
    guild_id: GuildId,
    user_id: UserId,
    reason: Option<String>,
}

impl RemoveGuildMember {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for RemoveGuildMember {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/guilds/{guild_id}/members/{user_id}",
            format!("/guilds/{}/members/{}", self.guild_id, self.user_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

super::into_future!(
    GetGuildMember,
    GetGuildMembers,
    SearchGuildMembers,
    GetGuildMemberPermissions,
    AddGuildMember,
    EditGuildMember,
    EditCurrentMember,
    EditCurrentUserNick,
    AddGuildMemberRole,
    RemoveGuildMemberRole,
    RemoveGuildMember,
);

impl Client {
    /// Gets a member of a guild.
    pub fn guild_member(&self, guild_id: GuildId, user_id: UserId) -> GetGuildMember {
        GetGuildMember {
            client: self.clone(),
            guild_id,
            user_id,
        }
    }

    /// Gets members of a guild, by user ID. Needs the `GUILD_MEMBERS` intent.
    pub fn guild_members(&self, guild_id: GuildId) -> GetGuildMembers {
        GetGuildMembers {
            client: self.clone(),
            guild_id,
            limit: None,
            after: None,
        }
    }

    /// Gets members whose username or nickname starts with `query`.
    pub fn search_guild_members(
        &self,
        guild_id: GuildId,
        query: impl Into<String>,
    ) -> SearchGuildMembers {
        SearchGuildMembers {
            client: self.clone(),
            guild_id,
            query: query.into(),
            limit: None,
        }
    }

    /// Gets the guild permissions of a member, from its roles and `@everyone`.
    pub fn guild_member_permissions(
        &self,
        guild_id: GuildId,
        user_id: UserId,
    ) -> GetGuildMemberPermissions {
        GetGuildMemberPermissions {
            client: self.clone(),
            guild_id,
            user_id,
        }
    }

    /// Adds a user to a guild with an OAuth2 access token that has the `guilds.join` scope.
    ///
    /// The client must use the bot token of the same application. The bot needs
    /// `CREATE_INSTANT_INVITE`.
    pub fn add_guild_member(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        access_token: impl Into<String>,
    ) -> AddGuildMember {
        AddGuildMember {
            client: self.clone(),
            guild_id,
            user_id,
            body: AddGuildMemberBody {
                access_token: AccessToken(access_token.into()),
                nick: None,
                roles: None,
                mute: None,
                deaf: None,
            },
        }
    }

    /// Changes a member: nickname, roles, voice state, timeout, or flags.
    pub fn edit_guild_member(&self, guild_id: GuildId, user_id: UserId) -> EditGuildMember {
        EditGuildMember {
            client: self.clone(),
            guild_id,
            user_id,
            body: EditGuildMemberBody::default(),
            reason: None,
        }
    }

    /// Changes the current member: nickname, avatar, banner, or bio.
    pub fn edit_current_member(&self, guild_id: GuildId) -> EditCurrentMember {
        EditCurrentMember {
            client: self.clone(),
            guild_id,
            body: EditCurrentMemberBody::default(),
            reason: None,
        }
    }

    /// Changes the nickname of the current user.
    #[deprecated(note = "Discord deprecated this endpoint; use `edit_current_member`")]
    pub fn edit_current_user_nick(&self, guild_id: GuildId) -> EditCurrentUserNick {
        EditCurrentUserNick {
            client: self.clone(),
            guild_id,
            body: EditCurrentUserNickBody::default(),
            reason: None,
        }
    }

    /// Gives a role to a member. Needs `MANAGE_ROLES`.
    pub fn add_guild_member_role(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        role_id: RoleId,
    ) -> AddGuildMemberRole {
        AddGuildMemberRole {
            client: self.clone(),
            guild_id,
            user_id,
            role_id,
            reason: None,
        }
    }

    /// Takes a role from a member. Needs `MANAGE_ROLES`.
    pub fn remove_guild_member_role(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        role_id: RoleId,
    ) -> RemoveGuildMemberRole {
        RemoveGuildMemberRole {
            client: self.clone(),
            guild_id,
            user_id,
            role_id,
            reason: None,
        }
    }

    /// Removes (kicks) a member from a guild. Needs `KICK_MEMBERS`.
    pub fn remove_guild_member(&self, guild_id: GuildId, user_id: UserId) -> RemoveGuildMember {
        RemoveGuildMember {
            client: self.clone(),
            guild_id,
            user_id,
            reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn access_token_is_hidden_in_debug() {
        let token = AccessToken("secret".to_owned());

        assert!(!format!("{token:?}").contains("secret"));
    }
}
