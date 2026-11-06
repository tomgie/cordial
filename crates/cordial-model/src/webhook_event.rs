//! Webhook events: events that Discord sends to the "Event Webhooks URL" of an application.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Timestamp;
use crate::application::ApplicationIntegrationType;
use crate::guild::Guild;
use crate::id::{ApplicationId, LobbyId, MessageId};
use crate::monetization::Entitlement;
use crate::oauth2::Scope;
use crate::user::User;

/// The body of a webhook event request.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct WebhookEventPayload {
    /// The version of the format. Always 1 now.
    pub version: u8,
    /// The application ID.
    pub application_id: ApplicationId,
    /// The payload type.
    #[serde(rename = "type")]
    pub kind: WebhookPayloadType,
    /// The event. Absent for PING.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<WebhookEventBody>,
}

crate::discord_enum! {
    /// The type of a webhook event payload.
    pub enum WebhookPayloadType: u8 {
        /// A check from Discord. Answer with status 204.
        Ping = 0,
        /// An event.
        Event = 1,
    }
}

/// The event of a webhook event payload.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct WebhookEventBody {
    /// The event type.
    #[serde(rename = "type")]
    pub kind: WebhookEventType,
    /// When the event happened.
    pub timestamp: Timestamp,
    /// The data. The shape depends on the type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl WebhookEventBody {
    /// Decodes the data into a typed event.
    ///
    /// # Errors
    ///
    /// Gives an error when the data does not match the event type.
    pub fn decode(&self) -> Result<WebhookEvent, serde_json::Error> {
        let data = self.data.clone().unwrap_or(Value::Null);
        Ok(match &self.kind {
            WebhookEventType::ApplicationAuthorized => {
                WebhookEvent::ApplicationAuthorized(Box::new(serde_json::from_value(data)?))
            }
            WebhookEventType::ApplicationDeauthorized => {
                WebhookEvent::ApplicationDeauthorized(serde_json::from_value(data)?)
            }
            WebhookEventType::EntitlementCreate => {
                WebhookEvent::EntitlementCreate(Box::new(serde_json::from_value(data)?))
            }
            WebhookEventType::EntitlementUpdate => {
                WebhookEvent::EntitlementUpdate(Box::new(serde_json::from_value(data)?))
            }
            WebhookEventType::EntitlementDelete => {
                WebhookEvent::EntitlementDelete(Box::new(serde_json::from_value(data)?))
            }
            WebhookEventType::LobbyMessageDelete => {
                WebhookEvent::LobbyMessageDelete(serde_json::from_value(data)?)
            }
            other => WebhookEvent::Other {
                kind: other.clone(),
                data,
            },
        })
    }
}

crate::discord_string_enum! {
    /// The type of a webhook event.
    pub enum WebhookEventType {
        /// A user authorized the application to a guild or to the user account.
        ApplicationAuthorized = "APPLICATION_AUTHORIZED",
        /// A user deauthorized the application.
        ApplicationDeauthorized = "APPLICATION_DEAUTHORIZED",
        /// An entitlement was made.
        EntitlementCreate = "ENTITLEMENT_CREATE",
        /// An entitlement changed.
        EntitlementUpdate = "ENTITLEMENT_UPDATE",
        /// An entitlement was deleted.
        EntitlementDelete = "ENTITLEMENT_DELETE",
        /// A user was added to a quest. Not available now.
        QuestUserEnrollment = "QUEST_USER_ENROLLMENT",
        /// A message was made in a lobby.
        LobbyMessageCreate = "LOBBY_MESSAGE_CREATE",
        /// A message in a lobby changed.
        LobbyMessageUpdate = "LOBBY_MESSAGE_UPDATE",
        /// A message in a lobby was deleted.
        LobbyMessageDelete = "LOBBY_MESSAGE_DELETE",
        /// A direct message was made in a Social SDK session.
        GameDirectMessageCreate = "GAME_DIRECT_MESSAGE_CREATE",
        /// A direct message changed in a Social SDK session.
        GameDirectMessageUpdate = "GAME_DIRECT_MESSAGE_UPDATE",
        /// A direct message was deleted in a Social SDK session.
        GameDirectMessageDelete = "GAME_DIRECT_MESSAGE_DELETE",
    }
}

/// A decoded webhook event.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum WebhookEvent {
    /// A user authorized the application.
    ApplicationAuthorized(Box<ApplicationAuthorized>),
    /// A user deauthorized the application.
    ApplicationDeauthorized(ApplicationDeauthorized),
    /// An entitlement was made.
    EntitlementCreate(Box<Entitlement>),
    /// An entitlement changed.
    EntitlementUpdate(Box<Entitlement>),
    /// An entitlement was deleted.
    EntitlementDelete(Box<Entitlement>),
    /// A message in a lobby was deleted.
    LobbyMessageDelete(LobbyMessageDelete),
    /// Another event type, with its raw data. Social SDK message events are here.
    Other {
        /// The event type.
        kind: WebhookEventType,
        /// The raw data.
        data: Value,
    },
}

/// The data of `APPLICATION_AUTHORIZED`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ApplicationAuthorized {
    /// The installation context: guild (0) or user (1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integration_type: Option<ApplicationIntegrationType>,
    /// The user that authorized the application.
    pub user: User,
    /// The scopes that the user authorized.
    #[serde(default)]
    pub scopes: Vec<Scope>,
    /// The guild, for a guild installation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild: Option<Guild>,
}

/// The data of `APPLICATION_DEAUTHORIZED`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ApplicationDeauthorized {
    /// The user that deauthorized the application.
    pub user: User,
}

/// The data of `LOBBY_MESSAGE_DELETE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct LobbyMessageDelete {
    /// The message ID.
    pub id: MessageId,
    /// The lobby ID.
    pub lobby_id: LobbyId,
}
