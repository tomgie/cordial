//! Guild join request endpoints, for guilds with member applications.
//!
//! The Discord docs do not describe these endpoints. They come from the OpenAPI specification.

use std::fmt;

use cordial_model::Timestamp;
use cordial_model::id::{GenericId, GuildId, UserId};
use cordial_model::user::User;
use serde::{Deserialize, Serialize};

use super::limits;
use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route};

/// The maximum number of characters in a rejection reason.
const REJECTION_REASON_LIMIT: usize = 160;

/// The status of a join request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum JoinRequestStatus {
    /// The user started the application.
    Started,
    /// The user sent the application.
    Submitted,
    /// A moderator rejected the application.
    Rejected,
    /// A moderator approved the application.
    Approved,
    /// A status that this version of the library does not know.
    #[serde(other)]
    Unknown,
}

impl JoinRequestStatus {
    /// Gives the value that Discord uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Started => "STARTED",
            Self::Submitted => "SUBMITTED",
            Self::Rejected => "REJECTED",
            Self::Approved => "APPROVED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl fmt::Display for JoinRequestStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A request of a user to join a guild.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildJoinRequest {
    /// The join request ID.
    pub id: GenericId,
    /// When the user made the request.
    pub created_at: Timestamp,
    /// When a moderator reviewed the request.
    #[serde(default)]
    pub reviewed_at: Option<Timestamp>,
    /// The status.
    #[serde(default)]
    pub application_status: Option<JoinRequestStatus>,
    /// The reason for a rejection.
    #[serde(default)]
    pub rejection_reason: Option<String>,
    /// The guild ID.
    pub guild_id: GuildId,
    /// The user ID.
    pub user_id: UserId,
    /// The user.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// The answers of the user to the form fields. Each item has a `field_type`, a `label`,
    /// and a `response`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub form_responses: Option<Vec<serde_json::Value>>,
    /// The moderator that reviewed the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actioned_by_user: Option<User>,
}

/// A page of join requests.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildJoinRequestList {
    /// The total number of join requests that match.
    #[serde(default)]
    pub total: u64,
    /// The join requests.
    #[serde(default)]
    pub guild_join_requests: Vec<GuildJoinRequest>,
}

/// `GET /guilds/{guild.id}/requests`: the join requests of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildJoinRequests {
    client: Client,
    guild_id: GuildId,
    status: Option<JoinRequestStatus>,
    position: Option<(&'static str, GenericId)>,
    limit: Option<u8>,
}

impl GetGuildJoinRequests {
    /// Gets only join requests with this status.
    pub fn status(mut self, status: JoinRequestStatus) -> Self {
        self.status = Some(status);
        self
    }

    /// Gets join requests with an ID less than this one.
    pub fn before(mut self, request_id: GenericId) -> Self {
        self.position = Some(("before", request_id));
        self
    }

    /// Gets join requests with an ID greater than this one.
    pub fn after(mut self, request_id: GenericId) -> Self {
        self.position = Some(("after", request_id));
        self
    }

    /// Sets the number of join requests, 1 to 100.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 100));
        self
    }
}

impl Endpoint for GetGuildJoinRequests {
    type Response = GuildJoinRequestList;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("status", self.status);
        query.push_opt("limit", self.limit);
        if let Some((name, request_id)) = self.position {
            query.push(name, request_id);
        }
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/requests",
            format!("/guilds/{}/requests{}", self.guild_id, query.finish()),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `PATCH /guilds/{guild.id}/requests/{request.id}`: approves or rejects a join request.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildJoinRequest {
    client: Client,
    guild_id: GuildId,
    request_id: GenericId,
    body: EditGuildJoinRequestBody,
}

#[derive(Clone, Debug, Serialize)]
struct EditGuildJoinRequestBody {
    action: JoinRequestStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    rejection_reason: Option<String>,
}

impl EditGuildJoinRequest {
    /// Sets the reason for a rejection, up to 160 characters. The user sees it.
    pub fn rejection_reason(mut self, reason: impl Into<String>) -> Self {
        self.body.rejection_reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditGuildJoinRequest {
    type Response = GuildJoinRequest;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/requests/{request_id}",
            format!("/guilds/{}/requests/{}", self.guild_id, self.request_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            limits::optional_length(
                "rejection_reason",
                self.body.rejection_reason.as_deref(),
                0,
                REJECTION_REASON_LIMIT,
            )
        })?;
        RequestBody::json(&self.body)
    }
}

super::into_future!(GetGuildJoinRequests, EditGuildJoinRequest);

impl Client {
    /// Gets the join requests of a guild.
    pub fn guild_join_requests(&self, guild_id: GuildId) -> GetGuildJoinRequests {
        GetGuildJoinRequests {
            client: self.clone(),
            guild_id,
            status: None,
            position: None,
            limit: None,
        }
    }

    /// Approves or rejects a join request. Use [`JoinRequestStatus::Approved`] or
    /// [`JoinRequestStatus::Rejected`].
    pub fn edit_guild_join_request(
        &self,
        guild_id: GuildId,
        request_id: GenericId,
        action: JoinRequestStatus,
    ) -> EditGuildJoinRequest {
        EditGuildJoinRequest {
            client: self.clone(),
            guild_id,
            request_id,
            body: EditGuildJoinRequestBody {
                action,
                rejection_reason: None,
            },
        }
    }
}
