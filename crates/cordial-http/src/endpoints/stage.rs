//! Stage instance endpoints.

use cordial_model::id::{ChannelId, ScheduledEventId};
use cordial_model::voice::{StageInstance, StagePrivacyLevel};
use serde::Serialize;

use super::channel::check_length;
use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody, check};
use crate::route::{MajorParameter, Method, Route};

/// `POST /stage-instances`: makes a stage channel live.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateStageInstance {
    client: Client,
    body: CreateStageBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct CreateStageBody {
    channel_id: ChannelId,
    topic: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    privacy_level: Option<StagePrivacyLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    send_start_notification: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    guild_scheduled_event_id: Option<ScheduledEventId>,
}

impl CreateStageInstance {
    /// Sets the privacy level. The default is `GuildOnly`.
    pub fn privacy_level(mut self, level: StagePrivacyLevel) -> Self {
        self.body.privacy_level = Some(level);
        self
    }

    /// Notifies `@everyone` that the stage started. This needs the `MENTION_EVERYONE`
    /// permission.
    pub fn send_start_notification(mut self, send: bool) -> Self {
        self.body.send_start_notification = Some(send);
        self
    }

    /// Connects the stage instance to a scheduled event.
    pub fn scheduled_event(mut self, event_id: ScheduledEventId) -> Self {
        self.body.guild_scheduled_event_id = Some(event_id);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for CreateStageInstance {
    type Response = StageInstance;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/stage-instances",
            "/stage-instances".to_owned(),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || check_length("topic", &self.body.topic, 1, 120))?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /stage-instances/{channel.id}`: the stage instance of a stage channel.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetStageInstance {
    client: Client,
    channel_id: ChannelId,
}

impl Endpoint for GetStageInstance {
    type Response = StageInstance;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/stage-instances/{channel_id}",
            format!("/stage-instances/{}", self.channel_id),
            MajorParameter::None,
        )
    }
}

/// `PATCH /stage-instances/{channel.id}`: edits a stage instance.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditStageInstance {
    client: Client,
    channel_id: ChannelId,
    body: EditStageBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditStageBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    privacy_level: Option<StagePrivacyLevel>,
}

impl EditStageInstance {
    /// Sets the topic, 1 to 120 characters.
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.body.topic = Some(topic.into());
        self
    }

    /// Sets the privacy level.
    pub fn privacy_level(mut self, level: StagePrivacyLevel) -> Self {
        self.body.privacy_level = Some(level);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditStageInstance {
    type Response = StageInstance;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/stage-instances/{channel_id}",
            format!("/stage-instances/{}", self.channel_id),
            MajorParameter::None,
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || match &self.body.topic {
            Some(topic) => check_length("topic", topic, 1, 120),
            None => Ok(()),
        })?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /stage-instances/{channel.id}`: ends a stage.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteStageInstance {
    client: Client,
    channel_id: ChannelId,
    reason: Option<String>,
}

impl DeleteStageInstance {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for DeleteStageInstance {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/stage-instances/{channel_id}",
            format!("/stage-instances/{}", self.channel_id),
            MajorParameter::None,
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

super::into_future!(
    CreateStageInstance,
    GetStageInstance,
    EditStageInstance,
    DeleteStageInstance,
);

impl Client {
    /// Makes a stage channel live with a topic of 1 to 120 characters. The current user
    /// must be a moderator of the stage.
    pub fn create_stage_instance(
        &self,
        channel_id: ChannelId,
        topic: impl Into<String>,
    ) -> CreateStageInstance {
        CreateStageInstance {
            client: self.clone(),
            body: CreateStageBody {
                channel_id,
                topic: topic.into(),
                privacy_level: None,
                send_start_notification: None,
                guild_scheduled_event_id: None,
            },
            reason: None,
        }
    }

    /// Gets the stage instance of a stage channel.
    pub fn stage_instance(&self, channel_id: ChannelId) -> GetStageInstance {
        GetStageInstance {
            client: self.clone(),
            channel_id,
        }
    }

    /// Edits the stage instance of a stage channel.
    pub fn edit_stage_instance(&self, channel_id: ChannelId) -> EditStageInstance {
        EditStageInstance {
            client: self.clone(),
            channel_id,
            body: EditStageBody::default(),
            reason: None,
        }
    }

    /// Ends the stage of a stage channel.
    pub fn delete_stage_instance(&self, channel_id: ChannelId) -> DeleteStageInstance {
        DeleteStageInstance {
            client: self.clone(),
            channel_id,
            reason: None,
        }
    }
}
