//! Lobbies and application identity profiles (Discord Social SDK).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::channel::Channel;
use crate::id::{ApplicationId, LobbyId, UserId};

/// A lobby: a group of users with an optional linked channel.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Lobby {
    /// The lobby ID.
    pub id: LobbyId,
    /// The application that made the lobby.
    pub application_id: ApplicationId,
    /// Key and value pairs, up to 1000 characters in total.
    #[serde(default)]
    pub metadata: Option<HashMap<String, String>>,
    /// The members.
    #[serde(default)]
    pub members: Vec<LobbyMember>,
    /// The guild channel that is linked to the lobby.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linked_channel: Option<Box<Channel>>,
}

/// A member of a lobby.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct LobbyMember {
    /// The user ID.
    pub id: UserId,
    /// Key and value pairs, up to 1000 characters in total.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    /// The member flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<LobbyMemberFlags>,
    /// An extra display name, 1 to 80 characters, for example a character name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub additional_name: Option<String>,
}

crate::discord_flags! {
    /// Flags of a lobby member.
    pub struct LobbyMemberFlags: u64 as integer {
        /// The member can link a text channel to the lobby.
        const CAN_LINK_LOBBY = 1 << 0;
    }
}

/// An identity of a user in an external system.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ApplicationIdentity {
    /// The provider type of the external account.
    pub provider_type: String,
    /// An identifier that tells identities of one provider apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    /// The user ID in the external system.
    pub provider_issued_user_id: String,
}

/// The game profile of an application identity.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ApplicationIdentityProfile {
    /// The username in the external system.
    #[serde(default)]
    pub username: Option<String>,
    /// Data that the game defines. Discord stores it but does not use it.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    /// The game statistics.
    #[serde(default)]
    pub data: Option<ProfileData>,
}

/// The game statistics of a profile.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ProfileData {
    /// The fields that Discord defines.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<PrimaryProfileData>,
    /// Custom fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dynamic: Option<Vec<DynamicProfileField>>,
}

/// The game statistics fields that Discord defines.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PrimaryProfileData {
    /// The current season name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub season: Option<String>,
    /// The current rank name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank_name: Option<String>,
    /// The image of the current rank.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank_image: Option<ProfileMedia>,
    /// The highest rank.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highest_rank: Option<String>,
    /// The image of the highest rank.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highest_rank_image: Option<ProfileMedia>,
    /// The name of the featured character.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub featured_played_character: Option<String>,
    /// The image of the featured character.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub featured_played_character_image: Option<ProfileMedia>,
    /// The total playtime in hours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub playtime_hours: Option<f64>,
    /// The total number of wins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_wins: Option<u64>,
    /// The wins in the current period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_period_wins: Option<u64>,
    /// The total number of games.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_games: Option<u64>,
    /// The games in the current period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_period_games: Option<u64>,
    /// The total number of kills.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_kills: Option<u64>,
    /// The kills in the current period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_period_kills: Option<u64>,
    /// The total number of assists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_assists: Option<u64>,
    /// The assists in the current period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_period_assists: Option<u64>,
    /// The total number of deaths.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_deaths: Option<u64>,
    /// The deaths in the current period.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_period_deaths: Option<u64>,
}

/// A custom game statistics field.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct DynamicProfileField {
    /// The field type: 1 string, 2 number, 3 media.
    #[serde(rename = "type")]
    pub kind: u8,
    /// The field name.
    pub name: String,
    /// The value: a string, a number, or a media object, by type.
    pub value: serde_json::Value,
}

/// A media asset of a profile.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ProfileMedia {
    /// The URL of the asset.
    pub url: String,
}
