//! Invites.

use serde::{Deserialize, Serialize};

use crate::application::Application;
use crate::channel::ChannelType;
use crate::guild::{GuildFeature, NsfwLevel, PartialRole, VerificationLevel, WelcomeScreen};
use crate::id::{ChannelId, GuildId};
use crate::scheduled_event::ScheduledEvent;
use crate::user::User;
use crate::{ImageHash, Timestamp};

/// An invite to a guild or a group DM.
///
/// The metadata fields (`uses` to `created_at`) are set only on some endpoints.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Invite {
    /// The invite type.
    #[serde(rename = "type", default)]
    pub kind: InviteType,
    /// The invite code.
    pub code: String,
    /// The partial guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild: Option<InviteGuild>,
    /// The partial channel.
    #[serde(default)]
    pub channel: Option<InviteChannel>,
    /// The user that made the invite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inviter: Option<User>,
    /// The target type, for a voice channel invite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_type: Option<InviteTargetType>,
    /// The user whose stream the invite shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_user: Option<User>,
    /// The embedded application that the invite opens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_application: Option<Box<Application>>,
    /// The approximate number of online members. Only with `with_counts`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate_presence_count: Option<u64>,
    /// The approximate number of members. Only with `with_counts`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate_member_count: Option<u64>,
    /// When the invite expires.
    #[serde(default)]
    pub expires_at: Option<Timestamp>,
    /// The scheduled event of the invite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_scheduled_event: Option<ScheduledEvent>,
    /// The flags of a guild invite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<InviteFlags>,
    /// The roles that a user gets when the user accepts the invite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roles: Option<Vec<PartialRole>>,
    /// The number of uses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uses: Option<u32>,
    /// The maximum number of uses. 0 means no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_uses: Option<u32>,
    /// The time in seconds after which the invite expires. 0 means never.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_age: Option<u32>,
    /// Tells if the invite gives only temporary membership.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporary: Option<bool>,
    /// When the invite was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<Timestamp>,
}

impl Invite {
    /// Gives the invite URL, for example `https://discord.gg/0vCdhLbwjZZTWZLD`.
    #[must_use]
    pub fn url(&self) -> String {
        format!("https://discord.gg/{}", self.code)
    }
}

crate::discord_enum! {
    /// The type of an invite.
    pub enum InviteType: u8 {
        /// A guild invite.
        Guild = 0,
        /// A group DM invite.
        GroupDm = 1,
        /// A friend invite.
        Friend = 2,
    }
}

crate::discord_enum! {
    /// The target type of a voice channel invite.
    pub enum InviteTargetType: u8 {
        /// A stream.
        Stream = 1,
        /// An embedded application.
        EmbeddedApplication = 2,
    }
}

crate::discord_flags! {
    /// Flags of a guild invite.
    pub struct InviteFlags: u64 as integer {
        /// The invite is a guest invite for a voice channel.
        const IS_GUEST_INVITE = 1 << 0;
    }
}

/// The partial guild of an invite.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct InviteGuild {
    /// The guild ID.
    pub id: GuildId,
    /// The name.
    pub name: String,
    /// The splash hash.
    #[serde(default)]
    pub splash: Option<ImageHash>,
    /// The banner hash.
    #[serde(default)]
    pub banner: Option<ImageHash>,
    /// The description.
    #[serde(default)]
    pub description: Option<String>,
    /// The icon hash.
    #[serde(default)]
    pub icon: Option<ImageHash>,
    /// The features.
    #[serde(default)]
    pub features: Vec<GuildFeature>,
    /// The verification level.
    #[serde(default)]
    pub verification_level: VerificationLevel,
    /// The vanity URL code.
    #[serde(default)]
    pub vanity_url_code: Option<String>,
    /// The age-restriction level.
    #[serde(default)]
    pub nsfw_level: NsfwLevel,
    /// The number of boosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premium_subscription_count: Option<u32>,
    /// The welcome screen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub welcome_screen: Option<WelcomeScreen>,
}

/// The partial channel of an invite.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct InviteChannel {
    /// The channel ID.
    pub id: ChannelId,
    /// The name.
    #[serde(default)]
    pub name: Option<String>,
    /// The channel type.
    #[serde(rename = "type")]
    pub kind: ChannelType,
    /// The icon hash of a group DM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<ImageHash>,
    /// The recipients of a group DM (only usernames).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipients: Option<Vec<InviteRecipient>>,
}

/// A recipient of a group DM invite.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct InviteRecipient {
    /// The username.
    pub username: String,
}

crate::discord_enum! {
    /// The status of a job that changes the target users of an invite.
    pub enum InviteTargetUsersJobStatus: u8 {
        /// The status is not set.
        Unspecified = 0,
        /// The job runs.
        Processing = 1,
        /// The job is complete.
        Completed = 2,
        /// The job failed.
        Failed = 3,
    }
}

/// The state of a job that changes the target users of an invite.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct InviteTargetUsersJob {
    /// The status.
    pub status: InviteTargetUsersJobStatus,
    /// The total number of users in the job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_users: Option<u64>,
    /// The number of users that the job processed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub processed_users: Option<u64>,
    /// When the job was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<Timestamp>,
    /// When the job completed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<Timestamp>,
    /// The error message, when the job failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}
