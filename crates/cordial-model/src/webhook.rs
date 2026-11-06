//! Webhooks.

use serde::{Deserialize, Serialize};

use crate::ImageHash;
use crate::id::{ApplicationId, ChannelId, GuildId, WebhookId};
use crate::user::User;

/// A webhook.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Webhook {
    /// The webhook ID.
    pub id: WebhookId,
    /// The webhook type.
    #[serde(rename = "type")]
    pub kind: WebhookType,
    /// The guild of the webhook.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The channel of the webhook.
    #[serde(default)]
    pub channel_id: Option<ChannelId>,
    /// The user that made the webhook. Absent when a request uses the webhook token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// The default name.
    #[serde(default)]
    pub name: Option<String>,
    /// The default avatar hash.
    #[serde(default)]
    pub avatar: Option<ImageHash>,
    /// The secret token. Only for incoming webhooks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<crate::Secret>,
    /// The application that made the webhook.
    #[serde(default)]
    pub application_id: Option<ApplicationId>,
    /// The guild of the followed channel. Only for channel follower webhooks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_guild: Option<WebhookGuild>,
    /// The followed channel. Only for channel follower webhooks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_channel: Option<WebhookChannel>,
    /// The URL to execute the webhook. Only from the `webhook.incoming` OAuth2 flow.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

crate::discord_enum! {
    /// The type of a webhook.
    pub enum WebhookType: u8 {
        /// An incoming webhook. It posts messages with its token.
        Incoming = 1,
        /// A channel follower webhook. Discord uses it for channel following.
        ChannelFollower = 2,
        /// An application webhook. Discord uses it for interactions.
        Application = 3,
    }
}

/// The partial guild of a channel follower webhook.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct WebhookGuild {
    /// The guild ID.
    pub id: GuildId,
    /// The name.
    pub name: String,
    /// The icon hash.
    #[serde(default)]
    pub icon: Option<ImageHash>,
}

/// The partial channel of a channel follower webhook.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct WebhookChannel {
    /// The channel ID.
    pub id: ChannelId,
    /// The name.
    pub name: String,
}
