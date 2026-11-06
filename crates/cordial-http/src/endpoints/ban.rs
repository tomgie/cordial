//! Guild ban endpoints.

use cordial_model::guild::{Ban, BulkBanResult};
use cordial_model::id::{GuildId, UserId};
use serde::Serialize;

use super::limits;
use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route};

/// The maximum time of messages to delete with a ban, in seconds (7 days).
const DELETE_MESSAGE_SECONDS_MAX: u64 = 604_800;
/// The maximum number of users in a bulk ban.
const BULK_BAN_LIMIT: usize = 200;

/// `GET /guilds/{guild.id}/bans`: the bans of a guild, by user ID in ascending order.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildBans {
    client: Client,
    guild_id: GuildId,
    position: Option<(&'static str, UserId)>,
    limit: Option<u16>,
}

impl GetGuildBans {
    /// Gets bans of users with an ID less than this one.
    pub fn before(mut self, user_id: UserId) -> Self {
        self.position = Some(("before", user_id));
        self
    }

    /// Gets bans of users with an ID greater than this one.
    pub fn after(mut self, user_id: UserId) -> Self {
        self.position = Some(("after", user_id));
        self
    }

    /// Sets the number of bans, 1 to 1000. The default is 1000.
    pub fn limit(mut self, limit: u16) -> Self {
        self.limit = Some(limit.clamp(1, 1000));
        self
    }
}

impl Endpoint for GetGuildBans {
    type Response = Vec<Ban>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("limit", self.limit);
        if let Some((name, user_id)) = self.position {
            query.push(name, user_id);
        }
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/bans",
            format!("/guilds/{}/bans{}", self.guild_id, query.finish()),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/bans/{user.id}`: the ban of a user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildBan {
    client: Client,
    guild_id: GuildId,
    user_id: UserId,
}

impl Endpoint for GetGuildBan {
    type Response = Ban;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/bans/{user_id}",
            format!("/guilds/{}/bans/{}", self.guild_id, self.user_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

#[derive(Clone, Debug, Default, Serialize)]
struct BanBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    delete_message_seconds: Option<u32>,
}

/// `PUT /guilds/{guild.id}/bans/{user.id}`: bans a user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateGuildBan {
    client: Client,
    guild_id: GuildId,
    user_id: UserId,
    body: BanBody,
    reason: Option<String>,
}

impl CreateGuildBan {
    /// Deletes the messages of the user from the last seconds, 0 to 604800 (7 days).
    pub fn delete_message_seconds(mut self, seconds: u32) -> Self {
        self.body.delete_message_seconds = Some(seconds);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for CreateGuildBan {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/guilds/{guild_id}/bans/{user_id}",
            format!("/guilds/{}/bans/{}", self.guild_id, self.user_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            limits::optional_range(
                "delete_message_seconds",
                self.body.delete_message_seconds.map(u64::from),
                0,
                DELETE_MESSAGE_SECONDS_MAX,
            )
        })?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /guilds/{guild.id}/bans/{user.id}`: removes the ban of a user.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct RemoveGuildBan {
    client: Client,
    guild_id: GuildId,
    user_id: UserId,
    reason: Option<String>,
}

impl RemoveGuildBan {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for RemoveGuildBan {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/guilds/{guild_id}/bans/{user_id}",
            format!("/guilds/{}/bans/{}", self.guild_id, self.user_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `POST /guilds/{guild.id}/bulk-ban`: bans up to 200 users.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct BulkGuildBan {
    client: Client,
    guild_id: GuildId,
    body: BulkBanBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct BulkBanBody {
    user_ids: Vec<UserId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delete_message_seconds: Option<u32>,
}

impl BulkGuildBan {
    /// Deletes the messages of the users from the last seconds, 0 to 604800 (7 days).
    pub fn delete_message_seconds(mut self, seconds: u32) -> Self {
        self.body.delete_message_seconds = Some(seconds);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for BulkGuildBan {
    type Response = BulkBanResult;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/guilds/{guild_id}/bulk-ban",
            format!("/guilds/{}/bulk-ban", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            limits::count("user_ids", self.body.user_ids.len(), BULK_BAN_LIMIT)?;
            limits::optional_range(
                "delete_message_seconds",
                self.body.delete_message_seconds.map(u64::from),
                0,
                DELETE_MESSAGE_SECONDS_MAX,
            )
        })?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

super::into_future!(
    GetGuildBans,
    GetGuildBan,
    CreateGuildBan,
    RemoveGuildBan,
    BulkGuildBan
);

impl Client {
    /// Gets the bans of a guild. Needs `BAN_MEMBERS` or `VIEW_AUDIT_LOG`.
    pub fn guild_bans(&self, guild_id: GuildId) -> GetGuildBans {
        GetGuildBans {
            client: self.clone(),
            guild_id,
            position: None,
            limit: None,
        }
    }

    /// Gets the ban of a user. Needs `BAN_MEMBERS` or `VIEW_AUDIT_LOG`.
    pub fn guild_ban(&self, guild_id: GuildId, user_id: UserId) -> GetGuildBan {
        GetGuildBan {
            client: self.clone(),
            guild_id,
            user_id,
        }
    }

    /// Bans a user. Needs `BAN_MEMBERS`.
    pub fn ban_member(&self, guild_id: GuildId, user_id: UserId) -> CreateGuildBan {
        CreateGuildBan {
            client: self.clone(),
            guild_id,
            user_id,
            body: BanBody::default(),
            reason: None,
        }
    }

    /// Removes the ban of a user. Needs `BAN_MEMBERS`.
    pub fn unban_member(&self, guild_id: GuildId, user_id: UserId) -> RemoveGuildBan {
        RemoveGuildBan {
            client: self.clone(),
            guild_id,
            user_id,
            reason: None,
        }
    }

    /// Bans up to 200 users. Needs `BAN_MEMBERS` and `MANAGE_GUILD`.
    ///
    /// When no user can be banned, Discord gives an error.
    pub fn bulk_ban_members(&self, guild_id: GuildId, user_ids: Vec<UserId>) -> BulkGuildBan {
        BulkGuildBan {
            client: self.clone(),
            guild_id,
            body: BulkBanBody {
                user_ids,
                delete_message_seconds: None,
            },
            reason: None,
        }
    }
}
