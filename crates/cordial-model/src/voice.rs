//! Voice states, voice regions, and stage instances.

use serde::{Deserialize, Serialize};

use crate::Timestamp;
use crate::guild::Member;
use crate::id::{ChannelId, GuildId, ScheduledEventId, StageInstanceId, UserId};

/// The voice connection state of a user.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct VoiceState {
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The channel that the user is connected to. `None` when the user left.
    #[serde(default)]
    pub channel_id: Option<ChannelId>,
    /// The user ID.
    pub user_id: UserId,
    /// The member.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
    /// The session ID.
    pub session_id: String,
    /// Tells if the guild deafened the user.
    #[serde(default)]
    pub deaf: bool,
    /// Tells if the guild muted the user.
    #[serde(default)]
    pub mute: bool,
    /// Tells if the user deafened themselves.
    #[serde(default)]
    pub self_deaf: bool,
    /// Tells if the user muted themselves.
    #[serde(default)]
    pub self_mute: bool,
    /// Tells if the user streams with Go Live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub self_stream: Option<bool>,
    /// Tells if the camera of the user is on.
    #[serde(default)]
    pub self_video: bool,
    /// Tells if the user cannot speak (in stage channels).
    #[serde(default)]
    pub suppress: bool,
    /// When the user asked to speak.
    #[serde(default)]
    pub request_to_speak_timestamp: Option<Timestamp>,
}

/// A voice region.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct VoiceRegion {
    /// The region ID.
    pub id: String,
    /// The name.
    pub name: String,
    /// Tells if this is the closest region to the current user.
    #[serde(default)]
    pub optimal: bool,
    /// Tells if the region is deprecated. Do not switch to it.
    #[serde(default)]
    pub deprecated: bool,
    /// Tells if the region is a custom region, for example for events.
    #[serde(default)]
    pub custom: bool,
}

/// A live stage in a stage channel.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct StageInstance {
    /// The stage instance ID.
    pub id: StageInstanceId,
    /// The guild ID.
    pub guild_id: GuildId,
    /// The stage channel ID.
    pub channel_id: ChannelId,
    /// The topic, 1 to 120 characters.
    pub topic: String,
    /// The privacy level.
    #[serde(default)]
    pub privacy_level: StagePrivacyLevel,
    /// Deprecated.
    #[serde(default)]
    pub discoverable_disabled: bool,
    /// The scheduled event of the stage.
    #[serde(default)]
    pub guild_scheduled_event_id: Option<ScheduledEventId>,
}

crate::discord_enum! {
    /// The privacy level of a stage instance.
    pub enum StagePrivacyLevel: u8 {
        /// Visible to the public. Deprecated.
        Public = 1,
        /// Visible only to guild members.
        GuildOnly = 2,
    }
}
