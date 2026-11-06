//! User endpoints.
//!
//! Some of these endpoints accept only an OAuth2 Bearer token. Make the client with
//! [`Token::bearer`](crate::Token::bearer) for them:
//!
//! | Endpoint | Token | Scope |
//! |---|---|---|
//! | [`GetCurrentUserGuildMember`] | Bearer | `guilds.members.read` |
//! | [`GetCurrentUserConnections`] | Bearer (or bot) | `connections` |
//! | [`GetCurrentUserApplicationRoleConnection`] | Bearer | `role_connections.write` |
//! | [`UpdateCurrentUserApplicationRoleConnection`] | Bearer | `role_connections.write` |
//! | [`DeleteCurrentUserApplicationRoleConnection`] | Bearer | `role_connections.write` |

use std::collections::HashMap;

use cordial_model::Patch;
use cordial_model::channel::Channel;
use cordial_model::guild::{CurrentUserGuild, Member};
use cordial_model::id::{ApplicationId, GuildId, UserId};
use cordial_model::user::{ApplicationRoleConnection, Connection, User};
use cordial_model::validate::ValidationError;
use serde::Serialize;

use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, ImageData, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route};

/// The maximum number of characters in `platform_name` of a role connection.
pub const PLATFORM_NAME_LIMIT: usize = 50;
/// The maximum number of characters in `platform_username` of a role connection.
pub const PLATFORM_USERNAME_LIMIT: usize = 100;
/// The maximum number of characters in a metadata value of a role connection.
pub const ROLE_CONNECTION_METADATA_VALUE_LIMIT: usize = 100;

/// `GET /users/@me`: the current user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetCurrentUser {
    client: Client,
}

impl Endpoint for GetCurrentUser {
    type Response = User;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/users/@me",
            "/users/@me".to_owned(),
            MajorParameter::None,
        )
    }
}

/// `GET /users/{user.id}`: a user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetUser {
    client: Client,
    user_id: UserId,
}

impl Endpoint for GetUser {
    type Response = User;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/users/{user_id}",
            format!("/users/{}", self.user_id),
            MajorParameter::None,
        )
    }
}

/// `PATCH /users/@me`: changes the username, the avatar, or the banner of the current user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditCurrentUser {
    client: Client,
    body: EditCurrentUserBody,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditCurrentUserBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    avatar: Patch<ImageData>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    banner: Patch<ImageData>,
}

impl EditCurrentUser {
    /// Sets the username. A new username can change the discriminator.
    pub fn username(mut self, username: impl Into<String>) -> Self {
        self.body.username = Some(username.into());
        self
    }

    /// Sets the avatar.
    pub fn avatar(mut self, avatar: ImageData) -> Self {
        self.body.avatar = Patch::Set(avatar);
        self
    }

    /// Removes the avatar.
    pub fn clear_avatar(mut self) -> Self {
        self.body.avatar = Patch::Clear;
        self
    }

    /// Sets the banner.
    pub fn banner(mut self, banner: ImageData) -> Self {
        self.body.banner = Patch::Set(banner);
        self
    }

    /// Removes the banner.
    pub fn clear_banner(mut self) -> Self {
        self.body.banner = Patch::Clear;
        self
    }
}

impl Endpoint for EditCurrentUser {
    type Response = User;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/users/@me",
            "/users/@me".to_owned(),
            MajorParameter::None,
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }
}

/// `GET /users/@me/guilds`: partial guilds of the current user.
///
/// One request gives up to 200 guilds. A bot that uses large bot sharding must set
/// [`shard`](Self::shard). It then gets only the guilds of that shard.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetCurrentUserGuilds {
    client: Client,
    before: Option<GuildId>,
    after: Option<GuildId>,
    limit: Option<u8>,
    shard: Option<u32>,
    with_counts: Option<bool>,
}

impl GetCurrentUserGuilds {
    /// Gets guilds before a guild ID.
    pub fn before(mut self, guild_id: GuildId) -> Self {
        self.before = Some(guild_id);
        self
    }

    /// Gets guilds after a guild ID.
    pub fn after(mut self, guild_id: GuildId) -> Self {
        self.after = Some(guild_id);
        self
    }

    /// Sets the number of guilds, 1 to 200. The default is 200.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 200));
        self
    }

    /// Gets only the guilds of one shard, from 0 to `max_concurrency - 1`.
    ///
    /// Bots that use large bot sharding must send this value. `max_concurrency` comes from
    /// the session start limit of `GET /gateway/bot`. This value is not the gateway shard ID.
    pub fn shard(mut self, shard: u32) -> Self {
        self.shard = Some(shard);
        self
    }

    /// Adds the approximate member and presence counts to each guild.
    pub fn with_counts(mut self, with_counts: bool) -> Self {
        self.with_counts = Some(with_counts);
        self
    }
}

impl Endpoint for GetCurrentUserGuilds {
    type Response = Vec<CurrentUserGuild>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("before", self.before);
        query.push_opt("after", self.after);
        query.push_opt("limit", self.limit);
        query.push_opt("shard", self.shard);
        query.push_opt("with_counts", self.with_counts);
        Route::new(
            Method::Get,
            "/users/@me/guilds",
            format!("/users/@me/guilds{}", query.finish()),
            MajorParameter::None,
        )
    }
}

/// `GET /users/@me/guilds/{guild.id}/member`: the member object of the current user.
///
/// This endpoint needs a Bearer token with the `guilds.members.read` scope.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetCurrentUserGuildMember {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetCurrentUserGuildMember {
    type Response = Member;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/users/@me/guilds/{guild_id}/member",
            format!("/users/@me/guilds/{}/member", self.guild_id),
            MajorParameter::None,
        )
    }
}

/// `DELETE /users/@me/guilds/{guild.id}`: the current user leaves a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct LeaveGuild {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for LeaveGuild {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/users/@me/guilds/{guild_id}",
            format!("/users/@me/guilds/{}", self.guild_id),
            MajorParameter::None,
        )
    }
}

/// `POST /users/@me/channels`: opens a DM channel with a user.
///
/// If the DM channel exists, Discord gives it. Open DMs only after an action of the user.
/// Many new DMs in a short time can cause rate limits or a block.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateDm {
    client: Client,
    recipient_id: UserId,
}

#[derive(Serialize)]
struct CreateDmBody {
    recipient_id: UserId,
}

impl Endpoint for CreateDm {
    type Response = Channel;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/users/@me/channels",
            "/users/@me/channels".to_owned(),
            MajorParameter::None,
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&CreateDmBody {
            recipient_id: self.recipient_id,
        })
    }

    fn retry_safe(&self) -> bool {
        // Discord gives the same channel for a repeated request.
        true
    }
}

/// `POST /users/@me/channels`: makes a group DM channel with users.
///
/// Each access token comes from a user that gave the `gdm.join` scope to the application.
/// An application can have up to 10 active group DMs.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateGroupDm {
    client: Client,
    body: CreateGroupDmBody,
}

#[derive(Clone, Debug, Serialize)]
struct CreateGroupDmBody {
    access_tokens: Vec<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    nicks: HashMap<UserId, String>,
}

impl CreateGroupDm {
    /// Sets the nickname of a user in the group DM.
    pub fn nick(mut self, user_id: UserId, nick: impl Into<String>) -> Self {
        self.body.nicks.insert(user_id, nick.into());
        self
    }
}

impl Endpoint for CreateGroupDm {
    type Response = Channel;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/users/@me/channels",
            "/users/@me/channels".to_owned(),
            MajorParameter::None,
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }
}

/// `GET /users/@me/connections`: the accounts on other services that the user connected.
///
/// With a Bearer token, this endpoint needs the `connections` scope.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetCurrentUserConnections {
    client: Client,
}

impl Endpoint for GetCurrentUserConnections {
    type Response = Vec<Connection>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/users/@me/connections",
            "/users/@me/connections".to_owned(),
            MajorParameter::None,
        )
    }
}

/// `GET /users/@me/applications/{application.id}/role-connection`: the role connection of
/// the user for an application.
///
/// This endpoint needs a Bearer token with the `role_connections.write` scope.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetCurrentUserApplicationRoleConnection {
    client: Client,
    application_id: ApplicationId,
}

impl Endpoint for GetCurrentUserApplicationRoleConnection {
    type Response = ApplicationRoleConnection;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/users/@me/applications/{application_id}/role-connection",
            format!(
                "/users/@me/applications/{}/role-connection",
                self.application_id
            ),
            MajorParameter::None,
        )
    }
}

/// `PUT /users/@me/applications/{application.id}/role-connection`: sets the role connection
/// of the user for an application.
///
/// This endpoint needs a Bearer token with the `role_connections.write` scope.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct UpdateCurrentUserApplicationRoleConnection {
    client: Client,
    application_id: ApplicationId,
    body: RoleConnectionBody,
}

#[derive(Clone, Debug, Default, Serialize)]
struct RoleConnectionBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    platform_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    platform_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<HashMap<String, String>>,
}

impl UpdateCurrentUserApplicationRoleConnection {
    /// Sets the name of the platform, up to 50 characters.
    pub fn platform_name(mut self, name: impl Into<String>) -> Self {
        self.body.platform_name = Some(name.into());
        self
    }

    /// Sets the username on the platform, up to 100 characters.
    pub fn platform_username(mut self, username: impl Into<String>) -> Self {
        self.body.platform_username = Some(username.into());
        self
    }

    /// Sets one metadata value. The key is a key of the role connection metadata records.
    ///
    /// Write integers as text, booleans as `"1"` or `"0"`, and dates in ISO 8601.
    pub fn metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.body
            .metadata
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }

    fn check_limits(&self) -> Result<(), ValidationError> {
        check_chars(
            "platform_name",
            self.body.platform_name.as_deref(),
            PLATFORM_NAME_LIMIT,
        )?;
        check_chars(
            "platform_username",
            self.body.platform_username.as_deref(),
            PLATFORM_USERNAME_LIMIT,
        )?;
        for (key, value) in self.body.metadata.iter().flatten() {
            check_chars(
                &format!("metadata.{key}"),
                Some(value),
                ROLE_CONNECTION_METADATA_VALUE_LIMIT,
            )?;
        }
        Ok(())
    }
}

fn check_chars(field: &str, text: Option<&str>, max: usize) -> Result<(), ValidationError> {
    let count = text.map_or(0, |text| text.chars().count());
    if count > max {
        return Err(ValidationError::new(
            field,
            format!("has {count} characters; the maximum is {max}"),
        ));
    }
    Ok(())
}

impl Endpoint for UpdateCurrentUserApplicationRoleConnection {
    type Response = ApplicationRoleConnection;

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/users/@me/applications/{application_id}/role-connection",
            format!(
                "/users/@me/applications/{}/role-connection",
                self.application_id
            ),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.check_limits())?;
        RequestBody::json(&self.body)
    }
}

/// `DELETE /users/@me/applications/{application.id}/role-connection`: removes the role
/// connection of the user for an application.
///
/// This endpoint needs a Bearer token with the `role_connections.write` scope.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteCurrentUserApplicationRoleConnection {
    client: Client,
    application_id: ApplicationId,
}

impl Endpoint for DeleteCurrentUserApplicationRoleConnection {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/users/@me/applications/{application_id}/role-connection",
            format!(
                "/users/@me/applications/{}/role-connection",
                self.application_id
            ),
            MajorParameter::None,
        )
    }
}

super::into_future!(
    GetCurrentUser,
    GetUser,
    EditCurrentUser,
    GetCurrentUserGuilds,
    GetCurrentUserGuildMember,
    LeaveGuild,
    CreateDm,
    CreateGroupDm,
    GetCurrentUserConnections,
    GetCurrentUserApplicationRoleConnection,
    UpdateCurrentUserApplicationRoleConnection,
    DeleteCurrentUserApplicationRoleConnection,
);

impl Client {
    /// Gets the current user.
    pub fn current_user(&self) -> GetCurrentUser {
        GetCurrentUser {
            client: self.clone(),
        }
    }

    /// Gets a user.
    pub fn user(&self, user_id: UserId) -> GetUser {
        GetUser {
            client: self.clone(),
            user_id,
        }
    }

    /// Changes the username, the avatar, or the banner of the current user.
    pub fn edit_current_user(&self) -> EditCurrentUser {
        EditCurrentUser {
            client: self.clone(),
            body: EditCurrentUserBody::default(),
        }
    }

    /// Gets the partial guilds of the current user.
    pub fn current_user_guilds(&self) -> GetCurrentUserGuilds {
        GetCurrentUserGuilds {
            client: self.clone(),
            before: None,
            after: None,
            limit: None,
            shard: None,
            with_counts: None,
        }
    }

    /// Gets the member object of the current user in a guild. Needs a Bearer token.
    pub fn current_user_guild_member(&self, guild_id: GuildId) -> GetCurrentUserGuildMember {
        GetCurrentUserGuildMember {
            client: self.clone(),
            guild_id,
        }
    }

    /// Makes the current user leave a guild.
    pub fn leave_guild(&self, guild_id: GuildId) -> LeaveGuild {
        LeaveGuild {
            client: self.clone(),
            guild_id,
        }
    }

    /// Opens a DM channel with a user, or gets the open one.
    pub fn create_dm(&self, recipient_id: UserId) -> CreateDm {
        CreateDm {
            client: self.clone(),
            recipient_id,
        }
    }

    /// Makes a group DM with the users of the access tokens (`gdm.join` scope).
    pub fn create_group_dm<I, S>(&self, access_tokens: I) -> CreateGroupDm
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        CreateGroupDm {
            client: self.clone(),
            body: CreateGroupDmBody {
                access_tokens: access_tokens.into_iter().map(Into::into).collect(),
                nicks: HashMap::new(),
            },
        }
    }

    /// Gets the connections of the current user.
    pub fn current_user_connections(&self) -> GetCurrentUserConnections {
        GetCurrentUserConnections {
            client: self.clone(),
        }
    }

    /// Gets the role connection of the current user for an application. Needs a Bearer token.
    pub fn current_user_role_connection(
        &self,
        application_id: ApplicationId,
    ) -> GetCurrentUserApplicationRoleConnection {
        GetCurrentUserApplicationRoleConnection {
            client: self.clone(),
            application_id,
        }
    }

    /// Sets the role connection of the current user for an application. Needs a Bearer token.
    pub fn update_current_user_role_connection(
        &self,
        application_id: ApplicationId,
    ) -> UpdateCurrentUserApplicationRoleConnection {
        UpdateCurrentUserApplicationRoleConnection {
            client: self.clone(),
            application_id,
            body: RoleConnectionBody::default(),
        }
    }

    /// Removes the role connection of the current user for an application. Needs a Bearer
    /// token.
    pub fn delete_current_user_role_connection(
        &self,
        application_id: ApplicationId,
    ) -> DeleteCurrentUserApplicationRoleConnection {
        DeleteCurrentUserApplicationRoleConnection {
            client: self.clone(),
            application_id,
        }
    }
}
