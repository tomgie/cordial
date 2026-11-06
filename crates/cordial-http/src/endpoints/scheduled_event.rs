//! Guild scheduled event endpoints.

use std::collections::HashMap;

use cordial_model::id::{ChannelId, GenericId, GuildId, ScheduledEventId, UserId};
use cordial_model::scheduled_event::{
    RecurrenceRule, ScheduledEvent, ScheduledEventEntityMetadata, ScheduledEventEntityType,
    ScheduledEventPrivacyLevel, ScheduledEventStatus, ScheduledEventUser,
};
use cordial_model::validate::ValidationError;
use cordial_model::{Patch, Timestamp};
use serde::{Deserialize, Serialize};

use super::limits;
use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, ImageData, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route};

/// The maximum number of characters in the name of an event.
const NAME_LIMIT: usize = 100;
/// The maximum number of characters in the description of an event.
const DESCRIPTION_LIMIT: usize = 1000;
/// The maximum number of characters in the location of an event.
const LOCATION_LIMIT: usize = 100;
/// The maximum number of exceptions in a user count request.
const EXCEPTION_COUNT_LIMIT: usize = 10;

/// `GET /guilds/{guild.id}/scheduled-events`: the scheduled events of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildScheduledEvents {
    client: Client,
    guild_id: GuildId,
    with_user_count: Option<bool>,
}

impl GetGuildScheduledEvents {
    /// Adds the number of subscribed users to each event when `true`.
    pub fn with_user_count(mut self, with_user_count: bool) -> Self {
        self.with_user_count = Some(with_user_count);
        self
    }
}

impl Endpoint for GetGuildScheduledEvents {
    type Response = Vec<ScheduledEvent>;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("with_user_count", self.with_user_count);
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/scheduled-events",
            format!(
                "/guilds/{}/scheduled-events{}",
                self.guild_id,
                query.finish()
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `POST /guilds/{guild.id}/scheduled-events`: makes a scheduled event.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateGuildScheduledEvent {
    client: Client,
    guild_id: GuildId,
    body: CreateGuildScheduledEventBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct CreateGuildScheduledEventBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    channel_id: Option<ChannelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entity_metadata: Option<ScheduledEventEntityMetadata>,
    name: String,
    privacy_level: ScheduledEventPrivacyLevel,
    scheduled_start_time: Timestamp,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduled_end_time: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    entity_type: ScheduledEventEntityType,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<ImageData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recurrence_rule: Option<RecurrenceRule>,
}

impl CreateGuildScheduledEvent {
    /// Sets the channel. Required for stage and voice events. Not allowed for external events.
    pub fn channel_id(mut self, channel_id: ChannelId) -> Self {
        self.body.channel_id = Some(channel_id);
        self
    }

    /// Sets the location, 1 to 100 characters. Required for external events.
    pub fn location(mut self, location: impl Into<String>) -> Self {
        self.body.entity_metadata = Some(ScheduledEventEntityMetadata::location(location));
        self
    }

    /// Sets the privacy level. The default and only value is
    /// [`ScheduledEventPrivacyLevel::GuildOnly`].
    pub fn privacy_level(mut self, level: ScheduledEventPrivacyLevel) -> Self {
        self.body.privacy_level = level;
        self
    }

    /// Sets the end time. Required for external events.
    pub fn scheduled_end_time(mut self, end: Timestamp) -> Self {
        self.body.scheduled_end_time = Some(end);
        self
    }

    /// Sets the description, 1 to 1000 characters.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.body.description = Some(description.into());
        self
    }

    /// Sets the cover image.
    pub fn image(mut self, image: ImageData) -> Self {
        self.body.image = Some(image);
        self
    }

    /// Sets how often the event recurs.
    pub fn recurrence_rule(mut self, rule: RecurrenceRule) -> Self {
        self.body.recurrence_rule = Some(rule);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl CreateGuildScheduledEventBody {
    /// Checks the field requirements of the entity type.
    fn check(&self) -> Result<(), ValidationError> {
        limits::length("name", &self.name, 1, NAME_LIMIT)?;
        limits::optional_length(
            "description",
            self.description.as_deref(),
            1,
            DESCRIPTION_LIMIT,
        )?;
        let location = self
            .entity_metadata
            .as_ref()
            .and_then(|metadata| metadata.location.as_deref());
        limits::optional_length("entity_metadata.location", location, 1, LOCATION_LIMIT)?;

        if self.entity_type == ScheduledEventEntityType::External {
            if self.channel_id.is_some() {
                return Err(ValidationError::new(
                    "channel_id",
                    "must be absent for an external event",
                ));
            }
            if location.is_none() {
                return Err(ValidationError::new(
                    "entity_metadata.location",
                    "is required for an external event",
                ));
            }
            if self.scheduled_end_time.is_none() {
                return Err(ValidationError::new(
                    "scheduled_end_time",
                    "is required for an external event",
                ));
            }
        } else if self.channel_id.is_none() && !self.entity_type.is_unknown() {
            return Err(ValidationError::new(
                "channel_id",
                "is required for a stage or voice event",
            ));
        }
        Ok(())
    }
}

impl Endpoint for CreateGuildScheduledEvent {
    type Response = ScheduledEvent;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/guilds/{guild_id}/scheduled-events",
            format!("/guilds/{}/scheduled-events", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || self.body.check())?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /guilds/{guild.id}/scheduled-events/{guild_scheduled_event.id}`: a scheduled event.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildScheduledEvent {
    client: Client,
    guild_id: GuildId,
    event_id: ScheduledEventId,
    with_user_count: Option<bool>,
}

impl GetGuildScheduledEvent {
    /// Adds the number of subscribed users when `true`.
    pub fn with_user_count(mut self, with_user_count: bool) -> Self {
        self.with_user_count = Some(with_user_count);
        self
    }
}

impl Endpoint for GetGuildScheduledEvent {
    type Response = ScheduledEvent;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("with_user_count", self.with_user_count);
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/scheduled-events/{event_id}",
            format!(
                "/guilds/{}/scheduled-events/{}{}",
                self.guild_id,
                self.event_id,
                query.finish()
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `PATCH /guilds/{guild.id}/scheduled-events/{guild_scheduled_event.id}`: changes a
/// scheduled event. Change the status to start or end the event.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildScheduledEvent {
    client: Client,
    guild_id: GuildId,
    event_id: ScheduledEventId,
    body: EditGuildScheduledEventBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditGuildScheduledEventBody {
    #[serde(skip_serializing_if = "Patch::is_keep")]
    channel_id: Patch<ChannelId>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    entity_metadata: Patch<ScheduledEventEntityMetadata>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    privacy_level: Option<ScheduledEventPrivacyLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduled_start_time: Option<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduled_end_time: Option<Timestamp>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    description: Patch<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entity_type: Option<ScheduledEventEntityType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<ScheduledEventStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<ImageData>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    recurrence_rule: Patch<RecurrenceRule>,
}

impl EditGuildScheduledEvent {
    /// Sets the channel.
    pub fn channel_id(mut self, channel_id: ChannelId) -> Self {
        self.body.channel_id = Patch::Set(channel_id);
        self
    }

    /// Removes the channel. Required when the entity type changes to external.
    pub fn clear_channel_id(mut self) -> Self {
        self.body.channel_id = Patch::Clear;
        self
    }

    /// Sets the location, 1 to 100 characters. Discord ignores it for events that are not
    /// external.
    pub fn location(mut self, location: impl Into<String>) -> Self {
        self.body.entity_metadata = Patch::Set(ScheduledEventEntityMetadata::location(location));
        self
    }

    /// Removes the entity metadata.
    pub fn clear_entity_metadata(mut self) -> Self {
        self.body.entity_metadata = Patch::Clear;
        self
    }

    /// Sets the name, 1 to 100 characters.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.body.name = Some(name.into());
        self
    }

    /// Sets the privacy level.
    pub fn privacy_level(mut self, level: ScheduledEventPrivacyLevel) -> Self {
        self.body.privacy_level = Some(level);
        self
    }

    /// Sets the start time.
    pub fn scheduled_start_time(mut self, start: Timestamp) -> Self {
        self.body.scheduled_start_time = Some(start);
        self
    }

    /// Sets the end time. Required when the entity type changes to external.
    pub fn scheduled_end_time(mut self, end: Timestamp) -> Self {
        self.body.scheduled_end_time = Some(end);
        self
    }

    /// Sets the description, 1 to 1000 characters.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.body.description = Patch::Set(description.into());
        self
    }

    /// Removes the description.
    pub fn clear_description(mut self) -> Self {
        self.body.description = Patch::Clear;
        self
    }

    /// Sets the entity type.
    ///
    /// For an external event, also clear the channel, and set a location and an end time.
    pub fn entity_type(mut self, entity_type: ScheduledEventEntityType) -> Self {
        self.body.entity_type = Some(entity_type);
        self
    }

    /// Sets the status. Valid changes: scheduled to active, active to completed, and
    /// scheduled to canceled.
    pub fn status(mut self, status: ScheduledEventStatus) -> Self {
        self.body.status = Some(status);
        self
    }

    /// Sets the cover image.
    pub fn image(mut self, image: ImageData) -> Self {
        self.body.image = Some(image);
        self
    }

    /// Sets how often the event recurs.
    pub fn recurrence_rule(mut self, rule: RecurrenceRule) -> Self {
        self.body.recurrence_rule = Patch::Set(rule);
        self
    }

    /// Stops the recurrence of the event.
    pub fn clear_recurrence_rule(mut self) -> Self {
        self.body.recurrence_rule = Patch::Clear;
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditGuildScheduledEvent {
    type Response = ScheduledEvent;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/scheduled-events/{event_id}",
            format!(
                "/guilds/{}/scheduled-events/{}",
                self.guild_id, self.event_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            let body = &self.body;
            limits::optional_length("name", body.name.as_deref(), 1, NAME_LIMIT)?;
            limits::optional_length(
                "description",
                limits::patch_text(&body.description),
                1,
                DESCRIPTION_LIMIT,
            )?;
            if let Patch::Set(metadata) = &body.entity_metadata {
                limits::optional_length(
                    "entity_metadata.location",
                    metadata.location.as_deref(),
                    1,
                    LOCATION_LIMIT,
                )?;
            }
            Ok(())
        })?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /guilds/{guild.id}/scheduled-events/{guild_scheduled_event.id}`: deletes a
/// scheduled event.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteGuildScheduledEvent {
    client: Client,
    guild_id: GuildId,
    event_id: ScheduledEventId,
}

impl Endpoint for DeleteGuildScheduledEvent {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/guilds/{guild_id}/scheduled-events/{event_id}",
            format!(
                "/guilds/{}/scheduled-events/{}",
                self.guild_id, self.event_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// The query of a request for the users of a scheduled event.
#[derive(Clone, Debug, Default)]
struct UsersQuery {
    limit: Option<u8>,
    with_member: Option<bool>,
    position: Option<(&'static str, UserId)>,
}

impl UsersQuery {
    fn finish(&self) -> String {
        let mut query = Query::new();
        query.push_opt("limit", self.limit);
        query.push_opt("with_member", self.with_member);
        if let Some((name, user_id)) = self.position {
            query.push(name, user_id);
        }
        query.finish()
    }
}

/// Adds the setters of the user list query to a request type with a `query: UsersQuery`
/// field.
macro_rules! users_query_setters {
    () => {
        /// Sets the number of users, 1 to 100. The default is 100.
        pub fn limit(mut self, limit: u8) -> Self {
            self.query.limit = Some(limit.clamp(1, 100));
            self
        }

        /// Adds the guild member of each user when `true`.
        pub fn with_member(mut self, with_member: bool) -> Self {
            self.query.with_member = Some(with_member);
            self
        }

        /// Gets users with an ID less than this one. Users are in ascending order of ID.
        pub fn before(mut self, user_id: UserId) -> Self {
            self.query.position = Some(("before", user_id));
            self
        }

        /// Gets users with an ID greater than this one. Users are in ascending order of ID.
        pub fn after(mut self, user_id: UserId) -> Self {
            self.query.position = Some(("after", user_id));
            self
        }
    };
}

/// `GET /guilds/{guild.id}/scheduled-events/{guild_scheduled_event.id}/users`: the users that
/// subscribed to a scheduled event.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildScheduledEventUsers {
    client: Client,
    guild_id: GuildId,
    event_id: ScheduledEventId,
    query: UsersQuery,
}

impl GetGuildScheduledEventUsers {
    users_query_setters!();
}

impl Endpoint for GetGuildScheduledEventUsers {
    type Response = Vec<ScheduledEventUser>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/scheduled-events/{event_id}/users",
            format!(
                "/guilds/{}/scheduled-events/{}/users{}",
                self.guild_id,
                self.event_id,
                self.query.finish()
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// The number of users that subscribed to a scheduled event and to its exceptions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ScheduledEventUserCounts {
    /// The number of users that subscribed to the event.
    pub guild_scheduled_event_count: u64,
    /// The number of users for each requested exception ID.
    #[serde(default)]
    pub guild_scheduled_event_exception_counts: HashMap<GenericId, u64>,
}

/// `GET /guilds/{guild.id}/scheduled-events/{guild_scheduled_event.id}/users/counts`: the
/// number of subscribed users.
///
/// The Discord docs do not describe this endpoint. It comes from the OpenAPI specification.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildScheduledEventUserCounts {
    client: Client,
    guild_id: GuildId,
    event_id: ScheduledEventId,
    exception_ids: Vec<GenericId>,
}

impl GetGuildScheduledEventUserCounts {
    /// Adds the counts of up to 10 exceptions. The request sends only the first 10 IDs.
    pub fn exception_ids(mut self, mut exception_ids: Vec<GenericId>) -> Self {
        exception_ids.truncate(EXCEPTION_COUNT_LIMIT);
        self.exception_ids = exception_ids;
        self
    }
}

impl Endpoint for GetGuildScheduledEventUserCounts {
    type Response = ScheduledEventUserCounts;

    fn route(&self) -> Route {
        let mut query = Query::new();
        for exception_id in &self.exception_ids {
            query.push("guild_scheduled_event_exception_ids", exception_id);
        }
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/scheduled-events/{event_id}/users/counts",
            format!(
                "/guilds/{}/scheduled-events/{}/users/counts{}",
                self.guild_id,
                self.event_id,
                query.finish()
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/scheduled-events/{guild_scheduled_event.id}/{exception.id}/users`:
/// the users that subscribed to one exception of a recurring event.
///
/// The Discord docs do not describe this endpoint. It comes from the OpenAPI specification.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildScheduledEventExceptionUsers {
    client: Client,
    guild_id: GuildId,
    event_id: ScheduledEventId,
    exception_id: GenericId,
    query: UsersQuery,
}

impl GetGuildScheduledEventExceptionUsers {
    users_query_setters!();
}

impl Endpoint for GetGuildScheduledEventExceptionUsers {
    type Response = Vec<ScheduledEventUser>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/scheduled-events/{event_id}/{exception_id}/users",
            format!(
                "/guilds/{}/scheduled-events/{}/{}/users{}",
                self.guild_id,
                self.event_id,
                self.exception_id,
                self.query.finish()
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// An exception of a recurring scheduled event: one occurrence that moved or was canceled.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ScheduledEventException {
    /// The event ID.
    pub event_id: ScheduledEventId,
    /// The exception ID.
    pub event_exception_id: GenericId,
    /// The new start time of the occurrence.
    #[serde(default)]
    pub scheduled_start_time: Option<Timestamp>,
    /// The new end time of the occurrence.
    #[serde(default)]
    pub scheduled_end_time: Option<Timestamp>,
    /// Tells if the occurrence is canceled.
    #[serde(default)]
    pub is_canceled: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
struct ExceptionBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    original_scheduled_start_time: Option<Timestamp>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    scheduled_start_time: Patch<Timestamp>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    scheduled_end_time: Patch<Timestamp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_canceled: Option<bool>,
}

/// Adds the setters of the exception fields to a request type with a `body: ExceptionBody`
/// field.
macro_rules! exception_setters {
    () => {
        /// Sets the new start time of the occurrence.
        pub fn scheduled_start_time(mut self, start: Timestamp) -> Self {
            self.body.scheduled_start_time = Patch::Set(start);
            self
        }

        /// Removes the new start time of the occurrence.
        pub fn clear_scheduled_start_time(mut self) -> Self {
            self.body.scheduled_start_time = Patch::Clear;
            self
        }

        /// Sets the new end time of the occurrence.
        pub fn scheduled_end_time(mut self, end: Timestamp) -> Self {
            self.body.scheduled_end_time = Patch::Set(end);
            self
        }

        /// Removes the new end time of the occurrence.
        pub fn clear_scheduled_end_time(mut self) -> Self {
            self.body.scheduled_end_time = Patch::Clear;
            self
        }

        /// Cancels the occurrence when `true`.
        pub fn canceled(mut self, canceled: bool) -> Self {
            self.body.is_canceled = Some(canceled);
            self
        }
    };
}

/// `POST /guilds/{guild.id}/scheduled-events/{guild_scheduled_event.id}/exceptions`: moves or
/// cancels one occurrence of a recurring event.
///
/// The Discord docs do not describe this endpoint. It comes from the OpenAPI specification.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateGuildScheduledEventException {
    client: Client,
    guild_id: GuildId,
    event_id: ScheduledEventId,
    body: ExceptionBody,
}

impl CreateGuildScheduledEventException {
    exception_setters!();
}

impl Endpoint for CreateGuildScheduledEventException {
    type Response = ScheduledEventException;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/guilds/{guild_id}/scheduled-events/{event_id}/exceptions",
            format!(
                "/guilds/{}/scheduled-events/{}/exceptions",
                self.guild_id, self.event_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }
}

/// `PATCH /guilds/{guild.id}/scheduled-events/{guild_scheduled_event.id}/exceptions/{exception.id}`:
/// changes an exception of a recurring event.
///
/// The Discord docs do not describe this endpoint. It comes from the OpenAPI specification.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildScheduledEventException {
    client: Client,
    guild_id: GuildId,
    event_id: ScheduledEventId,
    exception_id: GenericId,
    body: ExceptionBody,
}

impl EditGuildScheduledEventException {
    exception_setters!();
}

impl Endpoint for EditGuildScheduledEventException {
    type Response = ScheduledEventException;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/scheduled-events/{event_id}/exceptions/{exception_id}",
            format!(
                "/guilds/{}/scheduled-events/{}/exceptions/{}",
                self.guild_id, self.event_id, self.exception_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }
}

/// `DELETE /guilds/{guild.id}/scheduled-events/{guild_scheduled_event.id}/exceptions/{exception.id}`:
/// removes an exception of a recurring event.
///
/// The Discord docs do not describe this endpoint. It comes from the OpenAPI specification.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteGuildScheduledEventException {
    client: Client,
    guild_id: GuildId,
    event_id: ScheduledEventId,
    exception_id: GenericId,
}

impl Endpoint for DeleteGuildScheduledEventException {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/guilds/{guild_id}/scheduled-events/{event_id}/exceptions/{exception_id}",
            format!(
                "/guilds/{}/scheduled-events/{}/exceptions/{}",
                self.guild_id, self.event_id, self.exception_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

super::into_future!(
    GetGuildScheduledEvents,
    CreateGuildScheduledEvent,
    GetGuildScheduledEvent,
    EditGuildScheduledEvent,
    DeleteGuildScheduledEvent,
    GetGuildScheduledEventUsers,
    GetGuildScheduledEventUserCounts,
    GetGuildScheduledEventExceptionUsers,
    CreateGuildScheduledEventException,
    EditGuildScheduledEventException,
    DeleteGuildScheduledEventException,
);

impl Client {
    /// Gets the scheduled events of a guild.
    pub fn guild_scheduled_events(&self, guild_id: GuildId) -> GetGuildScheduledEvents {
        GetGuildScheduledEvents {
            client: self.clone(),
            guild_id,
            with_user_count: None,
        }
    }

    /// Makes a scheduled event. A guild can have up to 100 scheduled or active events.
    ///
    /// A stage or voice event needs a channel. An external event needs a location and an end
    /// time.
    pub fn create_guild_scheduled_event(
        &self,
        guild_id: GuildId,
        name: impl Into<String>,
        entity_type: ScheduledEventEntityType,
        scheduled_start_time: Timestamp,
    ) -> CreateGuildScheduledEvent {
        CreateGuildScheduledEvent {
            client: self.clone(),
            guild_id,
            body: CreateGuildScheduledEventBody {
                channel_id: None,
                entity_metadata: None,
                name: name.into(),
                privacy_level: ScheduledEventPrivacyLevel::GuildOnly,
                scheduled_start_time,
                scheduled_end_time: None,
                description: None,
                entity_type,
                image: None,
                recurrence_rule: None,
            },
            reason: None,
        }
    }

    /// Gets a scheduled event.
    pub fn guild_scheduled_event(
        &self,
        guild_id: GuildId,
        event_id: ScheduledEventId,
    ) -> GetGuildScheduledEvent {
        GetGuildScheduledEvent {
            client: self.clone(),
            guild_id,
            event_id,
            with_user_count: None,
        }
    }

    /// Changes a scheduled event. Change the status to start or end the event.
    pub fn edit_guild_scheduled_event(
        &self,
        guild_id: GuildId,
        event_id: ScheduledEventId,
    ) -> EditGuildScheduledEvent {
        EditGuildScheduledEvent {
            client: self.clone(),
            guild_id,
            event_id,
            body: EditGuildScheduledEventBody::default(),
            reason: None,
        }
    }

    /// Deletes a scheduled event.
    pub fn delete_guild_scheduled_event(
        &self,
        guild_id: GuildId,
        event_id: ScheduledEventId,
    ) -> DeleteGuildScheduledEvent {
        DeleteGuildScheduledEvent {
            client: self.clone(),
            guild_id,
            event_id,
        }
    }

    /// Gets the users that subscribed to a scheduled event.
    pub fn guild_scheduled_event_users(
        &self,
        guild_id: GuildId,
        event_id: ScheduledEventId,
    ) -> GetGuildScheduledEventUsers {
        GetGuildScheduledEventUsers {
            client: self.clone(),
            guild_id,
            event_id,
            query: UsersQuery::default(),
        }
    }

    /// Gets the number of users that subscribed to a scheduled event and its exceptions.
    pub fn guild_scheduled_event_user_counts(
        &self,
        guild_id: GuildId,
        event_id: ScheduledEventId,
    ) -> GetGuildScheduledEventUserCounts {
        GetGuildScheduledEventUserCounts {
            client: self.clone(),
            guild_id,
            event_id,
            exception_ids: Vec::new(),
        }
    }

    /// Gets the users that subscribed to one exception of a recurring event.
    pub fn guild_scheduled_event_exception_users(
        &self,
        guild_id: GuildId,
        event_id: ScheduledEventId,
        exception_id: GenericId,
    ) -> GetGuildScheduledEventExceptionUsers {
        GetGuildScheduledEventExceptionUsers {
            client: self.clone(),
            guild_id,
            event_id,
            exception_id,
            query: UsersQuery::default(),
        }
    }

    /// Moves or cancels one occurrence of a recurring event. `original_scheduled_start_time`
    /// is the start time of the occurrence before the change.
    pub fn create_guild_scheduled_event_exception(
        &self,
        guild_id: GuildId,
        event_id: ScheduledEventId,
        original_scheduled_start_time: Timestamp,
    ) -> CreateGuildScheduledEventException {
        CreateGuildScheduledEventException {
            client: self.clone(),
            guild_id,
            event_id,
            body: ExceptionBody {
                original_scheduled_start_time: Some(original_scheduled_start_time),
                ..ExceptionBody::default()
            },
        }
    }

    /// Changes an exception of a recurring event.
    pub fn edit_guild_scheduled_event_exception(
        &self,
        guild_id: GuildId,
        event_id: ScheduledEventId,
        exception_id: GenericId,
    ) -> EditGuildScheduledEventException {
        EditGuildScheduledEventException {
            client: self.clone(),
            guild_id,
            event_id,
            exception_id,
            body: ExceptionBody::default(),
        }
    }

    /// Removes an exception of a recurring event. The occurrence goes back to the rule.
    pub fn delete_guild_scheduled_event_exception(
        &self,
        guild_id: GuildId,
        event_id: ScheduledEventId,
        exception_id: GenericId,
    ) -> DeleteGuildScheduledEventException {
        DeleteGuildScheduledEventException {
            client: self.clone(),
            guild_id,
            event_id,
            exception_id,
        }
    }
}
