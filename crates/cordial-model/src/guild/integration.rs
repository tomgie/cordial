//! Guild integrations.

use serde::{Deserialize, Serialize};

use crate::ImageHash;
use crate::Timestamp;
use crate::id::{ApplicationId, IntegrationId, RoleId};
use crate::oauth2::Scope;
use crate::user::User;

/// An integration of a guild, for example a bot or a Twitch subscription.
///
/// Many fields are absent for bot integrations and in partial integrations.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Integration {
    /// The integration ID.
    pub id: IntegrationId,
    /// The name.
    #[serde(default)]
    pub name: String,
    /// The integration type.
    #[serde(rename = "type")]
    pub kind: IntegrationType,
    /// Tells if the integration is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// Tells if the integration syncs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syncing: Option<bool>,
    /// The role for subscribers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_id: Option<RoleId>,
    /// Tells if emoticons sync (Twitch only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_emoticons: Option<bool>,
    /// What happens when a subscription expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire_behavior: Option<IntegrationExpireBehavior>,
    /// The grace period in days before subscriptions expire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire_grace_period: Option<u32>,
    /// The user of the integration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// The account on the service.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<IntegrationAccount>,
    /// When the integration last synced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<Timestamp>,
    /// The number of subscribers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscriber_count: Option<u32>,
    /// Tells if the integration is revoked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked: Option<bool>,
    /// The application of a Discord integration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application: Option<IntegrationApplication>,
    /// The application ID. Only in the partial integrations of audit logs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
    /// The OAuth2 scopes that the application has.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scopes: Option<Vec<Scope>>,
}

crate::discord_string_enum! {
    /// The type of an integration.
    pub enum IntegrationType {
        /// Twitch.
        Twitch = "twitch",
        /// YouTube.
        YouTube = "youtube",
        /// A Discord application.
        Discord = "discord",
        /// A guild subscription.
        GuildSubscription = "guild_subscription",
    }
}

crate::discord_enum! {
    /// What happens when a subscription of an integration expires.
    pub enum IntegrationExpireBehavior: u8 {
        /// Remove the role.
        RemoveRole = 0,
        /// Kick the member.
        Kick = 1,
    }
}

/// The account of an integration on its service.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct IntegrationAccount {
    /// The account ID.
    pub id: String,
    /// The account name.
    pub name: String,
}

/// The application of an integration.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct IntegrationApplication {
    /// The application ID.
    pub id: ApplicationId,
    /// The name.
    pub name: String,
    /// The icon hash.
    #[serde(default)]
    pub icon: Option<ImageHash>,
    /// The description.
    #[serde(default)]
    pub description: String,
    /// The bot user of the application.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bot: Option<User>,
}
