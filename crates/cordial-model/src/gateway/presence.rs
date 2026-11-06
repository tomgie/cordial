//! Presences and activities.

use serde::{Deserialize, Serialize};

use crate::ImageHash;
use crate::Timestamp;
use crate::id::{ApplicationId, EmojiId, GuildId, UserId};

crate::discord_string_enum! {
    /// The online status of a user.
    pub enum Status {
        /// Online.
        Online = "online",
        /// Do not disturb.
        DoNotDisturb = "dnd",
        /// Away.
        Idle = "idle",
        /// Shown as offline. Only for presence updates that the bot sends.
        Invisible = "invisible",
        /// Offline.
        Offline = "offline",
    }
}

// The enum macro cannot derive `Default`.
#[allow(clippy::derivable_impls)]
impl Default for Status {
    fn default() -> Self {
        Self::Online
    }
}

/// The presence of a user in a guild, as in `PRESENCE_UPDATE` and `GUILD_CREATE`.
///
/// Discord does not validate the field types of presences. This type is lenient.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Presence {
    /// The user. Only `id` is sure to be set.
    pub user: PresenceUser,
    /// The guild ID. Absent in the presences of `GUILD_CREATE`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The status.
    #[serde(default)]
    pub status: Status,
    /// The activities. A custom status can be absent because of privacy settings.
    #[serde(default)]
    pub activities: Vec<Activity>,
    /// The status for each platform.
    #[serde(default)]
    pub client_status: ClientStatus,
}

/// The partial user of a presence.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PresenceUser {
    /// The user ID.
    pub id: UserId,
    /// The username, when it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// The display name, when it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_name: Option<String>,
    /// The discriminator, when it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<String>,
    /// The avatar hash, when it changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<ImageHash>,
}

/// The status of a user for each platform. An absent platform means offline.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ClientStatus {
    /// The status on desktop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub desktop: Option<Status>,
    /// The status on mobile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mobile: Option<Status>,
    /// The status on web (also bots).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web: Option<Status>,
    /// The status on virtual reality.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vr: Option<Status>,
}

/// An activity of a user. Bots can set only `name`, `kind`, `state`, and `url`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Activity {
    /// The name.
    pub name: String,
    /// The activity type.
    #[serde(rename = "type", default)]
    pub kind: ActivityType,
    /// The stream URL. Only twitch.tv and youtube.com URLs are valid for streaming.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// When the activity was added, in Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
    /// The start and end times.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<ActivityTimestamps>,
    /// The application of the game.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
    /// Which field shows in the status text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_display_type: Option<StatusDisplayType>,
    /// What the player does now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
    /// The URL of the details text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details_url: Option<String>,
    /// The party status, or the text of a custom status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// The URL of the state text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_url: Option<String>,
    /// The emoji of a custom status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<ActivityEmoji>,
    /// The party of the player.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub party: Option<ActivityParty>,
    /// The images and their hover texts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<ActivityAssets>,
    /// The secrets for joining and spectating.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets: Option<ActivitySecrets>,
    /// Tells if the activity is an instanced game session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<bool>,
    /// The activity flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<ActivityFlags>,
    /// The labels of the custom buttons. Bots receive only the labels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buttons: Option<Vec<ActivityButton>>,
}

impl Activity {
    /// Makes an activity that a bot can set.
    #[must_use]
    pub fn new(kind: ActivityType, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind,
            url: None,
            created_at: None,
            timestamps: None,
            application_id: None,
            status_display_type: None,
            details: None,
            details_url: None,
            state: None,
            state_url: None,
            emoji: None,
            party: None,
            assets: None,
            secrets: None,
            instance: None,
            flags: None,
            buttons: None,
        }
    }

    /// Makes a "Playing {name}" activity.
    #[must_use]
    pub fn playing(name: impl Into<String>) -> Self {
        Self::new(ActivityType::Playing, name)
    }

    /// Makes a "Listening to {name}" activity.
    #[must_use]
    pub fn listening(name: impl Into<String>) -> Self {
        Self::new(ActivityType::Listening, name)
    }

    /// Makes a "Watching {name}" activity.
    #[must_use]
    pub fn watching(name: impl Into<String>) -> Self {
        Self::new(ActivityType::Watching, name)
    }

    /// Makes a "Competing in {name}" activity.
    #[must_use]
    pub fn competing(name: impl Into<String>) -> Self {
        Self::new(ActivityType::Competing, name)
    }

    /// Makes a streaming activity. Only twitch.tv and youtube.com URLs are valid.
    #[must_use]
    pub fn streaming(name: impl Into<String>, url: impl Into<String>) -> Self {
        let mut activity = Self::new(ActivityType::Streaming, name);
        activity.url = Some(url.into());
        activity
    }

    /// Makes a custom status with text.
    #[must_use]
    pub fn custom(state: impl Into<String>) -> Self {
        let mut activity = Self::new(ActivityType::Custom, "Custom Status");
        activity.state = Some(state.into());
        activity
    }
}

crate::discord_enum! {
    /// The type of an activity.
    pub enum ActivityType: u8 {
        /// "Playing {name}".
        Playing = 0,
        /// "Streaming {details}".
        Streaming = 1,
        /// "Listening to {name}".
        Listening = 2,
        /// "Watching {name}".
        Watching = 3,
        /// "{emoji} {state}".
        Custom = 4,
        /// "Competing in {name}".
        Competing = 5,
    }
}

crate::discord_enum! {
    /// Which field of an activity shows in the status text.
    pub enum StatusDisplayType: u8 {
        /// The name.
        Name = 0,
        /// The state.
        State = 1,
        /// The details.
        Details = 2,
    }
}

/// The start and end times of an activity, in Unix milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ActivityTimestamps {
    /// When the activity started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<u64>,
    /// When the activity ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<u64>,
}

impl ActivityTimestamps {
    /// Gives the start time.
    #[must_use]
    pub fn start_time(&self) -> Option<Timestamp> {
        self.start
            .and_then(|millis| i64::try_from(millis).ok())
            .map(Timestamp::from_unix_millis)
    }
}

/// The emoji of a custom status.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ActivityEmoji {
    /// The name.
    pub name: String,
    /// The ID of a custom emoji.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<EmojiId>,
    /// Tells if the emoji is animated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animated: Option<bool>,
}

/// The party of a player.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ActivityParty {
    /// The party ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// The current and maximum size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<[u32; 2]>,
}

/// The images of an activity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ActivityAssets {
    /// The large image: an application asset ID or `mp:{image_id}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub large_image: Option<String>,
    /// The hover text of the large image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub large_text: Option<String>,
    /// The URL of the large image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub large_url: Option<String>,
    /// The small image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_image: Option<String>,
    /// The hover text of the small image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_text: Option<String>,
    /// The URL of the small image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub small_url: Option<String>,
    /// The banner image of a game invite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invite_cover_image: Option<String>,
}

/// The secrets of an activity.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ActivitySecrets {
    /// The secret to join a party.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join: Option<String>,
    /// The secret to spectate a game.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spectate: Option<String>,
    /// The secret of an instanced match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#match: Option<String>,
}

crate::discord_flags! {
    /// Flags of an activity.
    pub struct ActivityFlags: u64 as integer {
        /// An instanced game session.
        const INSTANCE = 1 << 0;
        /// Users can join.
        const JOIN = 1 << 1;
        /// Users can spectate.
        const SPECTATE = 1 << 2;
        /// Users can ask to join.
        const JOIN_REQUEST = 1 << 3;
        /// The activity syncs (for example Spotify).
        const SYNC = 1 << 4;
        /// The activity can be played.
        const PLAY = 1 << 5;
        /// The party is only for friends.
        const PARTY_PRIVACY_FRIENDS = 1 << 6;
        /// The party is only for the voice channel.
        const PARTY_PRIVACY_VOICE_CHANNEL = 1 << 7;
        /// The activity is embedded.
        const EMBEDDED = 1 << 8;
    }
}

/// A button of an activity. Bots receive it as a label string, users send label and URL.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum ActivityButton {
    /// Only the label, as bots receive it.
    Label(String),
    /// The label and the URL.
    Full {
        /// The text, 1 to 32 characters.
        label: String,
        /// The URL, 1 to 512 characters.
        url: String,
    },
}
