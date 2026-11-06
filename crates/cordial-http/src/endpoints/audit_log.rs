//! Audit log endpoints.

use cordial_model::audit_log::{AuditLog, AuditLogEvent};
use cordial_model::id::{AuditLogEntryId, GenericId, GuildId, UserId};

use crate::client::Client;
use crate::request::Endpoint;
use crate::route::{MajorParameter, Method, Query, Route};

/// `GET /guilds/{guild.id}/audit-logs`: the audit log of a guild. Needs `VIEW_AUDIT_LOG`.
///
/// With `before` or no position, entries come newest first. With `after`, they come oldest
/// first.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildAuditLog {
    client: Client,
    guild_id: GuildId,
    user_id: Option<UserId>,
    target_id: Option<GenericId>,
    action_type: Option<AuditLogEvent>,
    before: Option<AuditLogEntryId>,
    after: Option<AuditLogEntryId>,
    limit: Option<u8>,
}

impl GetGuildAuditLog {
    /// Gets only entries of actions by this user.
    pub fn user_id(mut self, user_id: UserId) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Gets only entries about this target. The OpenAPI specification has this filter; the
    /// docs do not.
    pub fn target_id(mut self, target_id: GenericId) -> Self {
        self.target_id = Some(target_id);
        self
    }

    /// Gets only entries of this action type.
    pub fn action_type(mut self, action_type: AuditLogEvent) -> Self {
        self.action_type = Some(action_type);
        self
    }

    /// Gets entries with an ID less than this one, newest first.
    pub fn before(mut self, entry_id: AuditLogEntryId) -> Self {
        self.before = Some(entry_id);
        self
    }

    /// Gets entries with an ID greater than this one, oldest first.
    pub fn after(mut self, entry_id: AuditLogEntryId) -> Self {
        self.after = Some(entry_id);
        self
    }

    /// Sets the number of entries, 1 to 100. The default is 50.
    pub fn limit(mut self, limit: u8) -> Self {
        self.limit = Some(limit.clamp(1, 100));
        self
    }
}

impl Endpoint for GetGuildAuditLog {
    type Response = AuditLog;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("user_id", self.user_id);
        query.push_opt("target_id", self.target_id);
        query.push_opt("action_type", self.action_type.map(AuditLogEvent::value));
        query.push_opt("before", self.before);
        query.push_opt("after", self.after);
        query.push_opt("limit", self.limit);
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/audit-logs",
            format!("/guilds/{}/audit-logs{}", self.guild_id, query.finish()),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

super::into_future!(GetGuildAuditLog);

impl Client {
    /// Gets the audit log of a guild. Needs `VIEW_AUDIT_LOG`.
    pub fn guild_audit_log(&self, guild_id: GuildId) -> GetGuildAuditLog {
        GetGuildAuditLog {
            client: self.clone(),
            guild_id,
            user_id: None,
            target_id: None,
            action_type: None,
            before: None,
            after: None,
            limit: None,
        }
    }
}
