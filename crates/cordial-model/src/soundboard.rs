//! Soundboard sounds.

use serde::{Deserialize, Serialize};

use crate::id::{EmojiId, GuildId, SoundboardSoundId};
use crate::user::User;

/// A sound of the soundboard: a default sound or a guild sound.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct SoundboardSound {
    /// The name.
    pub name: String,
    /// The sound ID.
    pub sound_id: SoundboardSoundId,
    /// The volume, from 0 to 1.
    #[serde(default = "full_volume")]
    pub volume: f64,
    /// The ID of a custom emoji.
    #[serde(default)]
    pub emoji_id: Option<EmojiId>,
    /// A Unicode emoji.
    #[serde(default)]
    pub emoji_name: Option<String>,
    /// The guild of a guild sound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// Tells if the sound can be used. It can be `false` after the guild loses Server Boosts.
    #[serde(default = "available")]
    pub available: bool,
    /// The user that made the sound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
}

fn full_volume() -> f64 {
    1.0
}

fn available() -> bool {
    true
}

/// The response of `GET /guilds/{id}/soundboard-sounds`.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct SoundboardSounds {
    /// The sounds.
    pub items: Vec<SoundboardSound>,
}
