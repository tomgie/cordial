//! Guild role endpoints.

use std::collections::HashMap;

use cordial_model::guild::{Role, RoleColors};
use cordial_model::id::{GuildId, RoleId};
use cordial_model::{Patch, Permissions};
use serde::Serialize;

use super::guild::ComputedPermissions;
use super::limits;
use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, ImageData, RequestBody, check};
use crate::route::{MajorParameter, Method, Route};

/// The maximum number of characters in a role name.
const ROLE_NAME_LIMIT: usize = 100;

/// `GET /guilds/{guild.id}/roles`: the roles of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildRoles {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildRoles {
    type Response = Vec<Role>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/roles",
            format!("/guilds/{}/roles", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/roles/{role.id}`: a role.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildRole {
    client: Client,
    guild_id: GuildId,
    role_id: RoleId,
}

impl Endpoint for GetGuildRole {
    type Response = Role;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/roles/{role_id}",
            format!("/guilds/{}/roles/{}", self.guild_id, self.role_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/permissions/role/{role.id}`: the permissions of a role together
/// with `@everyone`.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildRolePermissions {
    client: Client,
    guild_id: GuildId,
    role_id: RoleId,
}

impl Endpoint for GetGuildRolePermissions {
    type Response = ComputedPermissions;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/permissions/role/{role_id}",
            format!(
                "/guilds/{}/permissions/role/{}",
                self.guild_id, self.role_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/roles/member-counts`: the number of members of each role.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildRoleMemberCounts {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildRoleMemberCounts {
    /// The number of members for each role ID, without `@everyone`.
    type Response = HashMap<RoleId, u64>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/roles/member-counts",
            format!("/guilds/{}/roles/member-counts", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `POST /guilds/{guild.id}/roles`: makes a role.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateGuildRole {
    client: Client,
    guild_id: GuildId,
    body: CreateGuildRoleBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct CreateGuildRoleBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permissions: Option<Permissions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    colors: Option<RoleColors>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hoist: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<ImageData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unicode_emoji: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mentionable: Option<bool>,
}

impl CreateGuildRole {
    /// Sets the name, up to 100 characters. The default is "new role".
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.body.name = Some(name.into());
        self
    }

    /// Sets the permissions. The default is the permissions of `@everyone`.
    pub fn permissions(mut self, permissions: Permissions) -> Self {
        self.body.permissions = Some(permissions);
        self
    }

    /// Sets the colors.
    pub fn colors(mut self, colors: RoleColors) -> Self {
        self.body.colors = Some(colors);
        self
    }

    /// Shows the role separately in the member list.
    pub fn hoist(mut self, hoist: bool) -> Self {
        self.body.hoist = Some(hoist);
        self
    }

    /// Sets the icon. Needs the `ROLE_ICONS` feature.
    pub fn icon(mut self, icon: ImageData) -> Self {
        self.body.icon = Some(icon);
        self
    }

    /// Sets a Unicode emoji as the icon. Needs the `ROLE_ICONS` feature.
    pub fn unicode_emoji(mut self, emoji: impl Into<String>) -> Self {
        self.body.unicode_emoji = Some(emoji.into());
        self
    }

    /// Lets members mention the role.
    pub fn mentionable(mut self, mentionable: bool) -> Self {
        self.body.mentionable = Some(mentionable);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for CreateGuildRole {
    type Response = Role;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/guilds/{guild_id}/roles",
            format!("/guilds/{}/roles", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            limits::optional_length("name", self.body.name.as_deref(), 0, ROLE_NAME_LIMIT)
        })?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// A new position of one role, for [`Client::edit_guild_role_positions`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct RolePosition {
    id: RoleId,
    position: i32,
}

impl RolePosition {
    /// Makes a new position for a role. Roles with the same position sort by ID.
    #[must_use]
    pub const fn new(role_id: RoleId, position: i32) -> Self {
        Self {
            id: role_id,
            position,
        }
    }
}

/// `PATCH /guilds/{guild.id}/roles`: changes the positions of roles.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildRolePositions {
    client: Client,
    guild_id: GuildId,
    positions: Vec<RolePosition>,
    reason: Option<String>,
}

impl EditGuildRolePositions {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditGuildRolePositions {
    /// All roles of the guild.
    type Response = Vec<Role>;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/roles",
            format!("/guilds/{}/roles", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.positions)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `PATCH /guilds/{guild.id}/roles/{role.id}`: changes a role.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildRole {
    client: Client,
    guild_id: GuildId,
    role_id: RoleId,
    body: EditGuildRoleBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditGuildRoleBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permissions: Option<Permissions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    colors: Option<RoleColors>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hoist: Option<bool>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    icon: Patch<ImageData>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    unicode_emoji: Patch<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mentionable: Option<bool>,
}

impl EditGuildRole {
    /// Sets the name, up to 100 characters.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.body.name = Some(name.into());
        self
    }

    /// Sets the permissions.
    pub fn permissions(mut self, permissions: Permissions) -> Self {
        self.body.permissions = Some(permissions);
        self
    }

    /// Sets the colors.
    pub fn colors(mut self, colors: RoleColors) -> Self {
        self.body.colors = Some(colors);
        self
    }

    /// Shows the role separately in the member list.
    pub fn hoist(mut self, hoist: bool) -> Self {
        self.body.hoist = Some(hoist);
        self
    }

    /// Sets the icon. Needs the `ROLE_ICONS` feature.
    pub fn icon(mut self, icon: ImageData) -> Self {
        self.body.icon = Patch::Set(icon);
        self
    }

    /// Removes the icon.
    pub fn clear_icon(mut self) -> Self {
        self.body.icon = Patch::Clear;
        self
    }

    /// Sets a Unicode emoji as the icon. Needs the `ROLE_ICONS` feature.
    pub fn unicode_emoji(mut self, emoji: impl Into<String>) -> Self {
        self.body.unicode_emoji = Patch::Set(emoji.into());
        self
    }

    /// Removes the Unicode emoji.
    pub fn clear_unicode_emoji(mut self) -> Self {
        self.body.unicode_emoji = Patch::Clear;
        self
    }

    /// Lets members mention the role.
    pub fn mentionable(mut self, mentionable: bool) -> Self {
        self.body.mentionable = Some(mentionable);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditGuildRole {
    type Response = Role;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/roles/{role_id}",
            format!("/guilds/{}/roles/{}", self.guild_id, self.role_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            limits::optional_length("name", self.body.name.as_deref(), 0, ROLE_NAME_LIMIT)
        })?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /guilds/{guild.id}/roles/{role.id}`: deletes a role.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteGuildRole {
    client: Client,
    guild_id: GuildId,
    role_id: RoleId,
    reason: Option<String>,
}

impl DeleteGuildRole {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for DeleteGuildRole {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/guilds/{guild_id}/roles/{role_id}",
            format!("/guilds/{}/roles/{}", self.guild_id, self.role_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

super::into_future!(
    GetGuildRoles,
    GetGuildRole,
    GetGuildRolePermissions,
    GetGuildRoleMemberCounts,
    CreateGuildRole,
    EditGuildRolePositions,
    EditGuildRole,
    DeleteGuildRole,
);

impl Client {
    /// Gets the roles of a guild.
    pub fn guild_roles(&self, guild_id: GuildId) -> GetGuildRoles {
        GetGuildRoles {
            client: self.clone(),
            guild_id,
        }
    }

    /// Gets a role.
    pub fn guild_role(&self, guild_id: GuildId, role_id: RoleId) -> GetGuildRole {
        GetGuildRole {
            client: self.clone(),
            guild_id,
            role_id,
        }
    }

    /// Gets the permissions of a role together with `@everyone`. Other roles and member
    /// overrides do not count.
    pub fn guild_role_permissions(
        &self,
        guild_id: GuildId,
        role_id: RoleId,
    ) -> GetGuildRolePermissions {
        GetGuildRolePermissions {
            client: self.clone(),
            guild_id,
            role_id,
        }
    }

    /// Gets the number of members of each role, without `@everyone`.
    pub fn guild_role_member_counts(&self, guild_id: GuildId) -> GetGuildRoleMemberCounts {
        GetGuildRoleMemberCounts {
            client: self.clone(),
            guild_id,
        }
    }

    /// Makes a role. Needs `MANAGE_ROLES`.
    pub fn create_guild_role(&self, guild_id: GuildId) -> CreateGuildRole {
        CreateGuildRole {
            client: self.clone(),
            guild_id,
            body: CreateGuildRoleBody::default(),
            reason: None,
        }
    }

    /// Changes the positions of roles. Needs `MANAGE_ROLES`.
    pub fn edit_guild_role_positions(
        &self,
        guild_id: GuildId,
        positions: Vec<RolePosition>,
    ) -> EditGuildRolePositions {
        EditGuildRolePositions {
            client: self.clone(),
            guild_id,
            positions,
            reason: None,
        }
    }

    /// Changes a role. Needs `MANAGE_ROLES`.
    pub fn edit_guild_role(&self, guild_id: GuildId, role_id: RoleId) -> EditGuildRole {
        EditGuildRole {
            client: self.clone(),
            guild_id,
            role_id,
            body: EditGuildRoleBody::default(),
            reason: None,
        }
    }

    /// Deletes a role. Needs `MANAGE_ROLES`.
    pub fn delete_guild_role(&self, guild_id: GuildId, role_id: RoleId) -> DeleteGuildRole {
        DeleteGuildRole {
            client: self.clone(),
            guild_id,
            role_id,
            reason: None,
        }
    }
}
