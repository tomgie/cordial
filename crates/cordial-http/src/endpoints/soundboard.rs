//! Soundboard endpoints.

use std::fmt;

use cordial_model::Patch;
use cordial_model::id::{ChannelId, EmojiId, GuildId, SoundboardSoundId};
use cordial_model::soundboard::{SoundboardSound, SoundboardSounds};
use cordial_model::validate::ValidationError;
use serde::Serialize;

use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, RequestBody, check, encode_base64};
use crate::route::{MajorParameter, Method, Route};

/// Sound data for a request, as a data URI: `data:audio/mpeg;base64,...`.
///
/// Discord accepts MP3 and Ogg files up to 512 KB and 5.2 seconds.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct SoundData(String);

impl SoundData {
    /// Makes sound data from bytes and a media type, for example `audio/ogg`.
    #[must_use]
    pub fn new(content_type: &str, bytes: &[u8]) -> Self {
        Self(format!(
            "data:{content_type};base64,{}",
            encode_base64(bytes)
        ))
    }

    /// Makes sound data from bytes. The media type is `audio/ogg` when the bytes start with
    /// `OggS`, else `audio/mpeg`.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let content_type = if bytes.starts_with(b"OggS") {
            "audio/ogg"
        } else {
            "audio/mpeg"
        };
        Self::new(content_type, bytes)
    }

    /// Makes sound data from a complete data URI.
    #[must_use]
    pub fn from_data_uri(uri: impl Into<String>) -> Self {
        Self(uri.into())
    }

    /// Gives the data URI.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SoundData {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header = self.0.split(',').next().unwrap_or_default();
        write!(formatter, "SoundData({header},<{} bytes>)", self.0.len())
    }
}

/// Checks a sound name: 2 to 32 characters.
fn check_sound_name(name: &str) -> Result<(), ValidationError> {
    let length = name.chars().count();
    if !(2..=32).contains(&length) {
        return Err(ValidationError::new(
            "name",
            format!("has {length} characters; the range is 2 to 32"),
        ));
    }
    Ok(())
}

/// Checks a volume: 0 to 1.
fn check_volume(volume: f64) -> Result<(), ValidationError> {
    if !(0.0..=1.0).contains(&volume) {
        return Err(ValidationError::new("volume", "must be from 0 to 1"));
    }
    Ok(())
}

/// `POST /channels/{channel.id}/send-soundboard-sound`: plays a soundboard sound in the voice
/// channel of the current user.
///
/// The current user must be in the voice channel, and not muted, deafened, or suppressed.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct SendSoundboardSound {
    client: Client,
    channel_id: ChannelId,
    fields: SendSoundboardSoundBody,
}

#[derive(Clone, Debug, Serialize)]
struct SendSoundboardSoundBody {
    sound_id: SoundboardSoundId,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_guild_id: Option<GuildId>,
}

impl SendSoundboardSound {
    /// Sets the guild of the sound. It is necessary for a sound of a different guild.
    pub fn source_guild_id(mut self, guild_id: GuildId) -> Self {
        self.fields.source_guild_id = Some(guild_id);
        self
    }
}

impl Endpoint for SendSoundboardSound {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/channels/{channel_id}/send-soundboard-sound",
            format!("/channels/{}/send-soundboard-sound", self.channel_id),
            MajorParameter::Channel(self.channel_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.fields)
    }
}

/// `GET /soundboard-default-sounds`: the default sounds that all users can use.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetDefaultSoundboardSounds {
    client: Client,
}

impl Endpoint for GetDefaultSoundboardSounds {
    type Response = Vec<SoundboardSound>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/soundboard-default-sounds",
            "/soundboard-default-sounds".to_owned(),
            MajorParameter::None,
        )
    }
}

/// `GET /guilds/{guild.id}/soundboard-sounds`: the sounds of a guild.
///
/// The sounds have `user` when the current user has the `CREATE_GUILD_EXPRESSIONS` or
/// `MANAGE_GUILD_EXPRESSIONS` permission.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildSoundboardSounds {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildSoundboardSounds {
    type Response = SoundboardSounds;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/soundboard-sounds",
            format!("/guilds/{}/soundboard-sounds", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/soundboard-sounds/{sound.id}`: one sound of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildSoundboardSound {
    client: Client,
    guild_id: GuildId,
    sound_id: SoundboardSoundId,
}

impl Endpoint for GetGuildSoundboardSound {
    type Response = SoundboardSound;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/soundboard-sounds/{sound_id}",
            format!(
                "/guilds/{}/soundboard-sounds/{}",
                self.guild_id, self.sound_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `POST /guilds/{guild.id}/soundboard-sounds`: adds a sound to a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateGuildSoundboardSound {
    client: Client,
    guild_id: GuildId,
    fields: CreateSoundBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct CreateSoundBody {
    name: String,
    sound: SoundData,
    #[serde(skip_serializing_if = "Option::is_none")]
    volume: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    emoji_id: Option<EmojiId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    emoji_name: Option<String>,
}

impl CreateGuildSoundboardSound {
    /// Sets the volume, from 0 to 1. The default is 1.
    pub fn volume(mut self, volume: f64) -> Self {
        self.fields.volume = Some(volume);
        self
    }

    /// Sets a custom emoji for the sound.
    pub fn emoji_id(mut self, emoji_id: EmojiId) -> Self {
        self.fields.emoji_id = Some(emoji_id);
        self
    }

    /// Sets a Unicode emoji for the sound.
    pub fn emoji_name(mut self, emoji: impl Into<String>) -> Self {
        self.fields.emoji_name = Some(emoji.into());
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for CreateGuildSoundboardSound {
    type Response = SoundboardSound;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/guilds/{guild_id}/soundboard-sounds",
            format!("/guilds/{}/soundboard-sounds", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            check_sound_name(&self.fields.name)?;
            if let Some(volume) = self.fields.volume {
                check_volume(volume)?;
            }
            Ok(())
        })?;
        RequestBody::json(&self.fields)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `PATCH /guilds/{guild.id}/soundboard-sounds/{sound.id}`: changes a sound of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct ModifyGuildSoundboardSound {
    client: Client,
    guild_id: GuildId,
    sound_id: SoundboardSoundId,
    fields: ModifySoundBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct ModifySoundBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    volume: Patch<f64>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    emoji_id: Patch<EmojiId>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    emoji_name: Patch<String>,
}

impl ModifyGuildSoundboardSound {
    /// Sets the name (2 to 32 characters).
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.fields.name = Some(name.into());
        self
    }

    /// Sets the volume, from 0 to 1. `None` sets the default volume.
    pub fn volume(mut self, volume: Option<f64>) -> Self {
        self.fields.volume = volume.into();
        self
    }

    /// Sets the custom emoji. `None` removes it.
    pub fn emoji_id(mut self, emoji_id: Option<EmojiId>) -> Self {
        self.fields.emoji_id = emoji_id.into();
        self
    }

    /// Sets the Unicode emoji. `None` removes it.
    pub fn emoji_name(mut self, emoji: Option<String>) -> Self {
        self.fields.emoji_name = emoji.into();
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for ModifyGuildSoundboardSound {
    type Response = SoundboardSound;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/soundboard-sounds/{sound_id}",
            format!(
                "/guilds/{}/soundboard-sounds/{}",
                self.guild_id, self.sound_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            if let Some(name) = &self.fields.name {
                check_sound_name(name)?;
            }
            if let Patch::Set(volume) = self.fields.volume {
                check_volume(volume)?;
            }
            Ok(())
        })?;
        RequestBody::json(&self.fields)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `DELETE /guilds/{guild.id}/soundboard-sounds/{sound.id}`: deletes a sound of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteGuildSoundboardSound {
    client: Client,
    guild_id: GuildId,
    sound_id: SoundboardSoundId,
    reason: Option<String>,
}

impl DeleteGuildSoundboardSound {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for DeleteGuildSoundboardSound {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/guilds/{guild_id}/soundboard-sounds/{sound_id}",
            format!(
                "/guilds/{}/soundboard-sounds/{}",
                self.guild_id, self.sound_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

super::into_future!(
    SendSoundboardSound,
    GetDefaultSoundboardSounds,
    GetGuildSoundboardSounds,
    GetGuildSoundboardSound,
    CreateGuildSoundboardSound,
    ModifyGuildSoundboardSound,
    DeleteGuildSoundboardSound,
);

impl Client {
    /// Plays a soundboard sound in a voice channel that the current user is in.
    pub fn send_soundboard_sound(
        &self,
        channel_id: ChannelId,
        sound_id: SoundboardSoundId,
    ) -> SendSoundboardSound {
        SendSoundboardSound {
            client: self.clone(),
            channel_id,
            fields: SendSoundboardSoundBody {
                sound_id,
                source_guild_id: None,
            },
        }
    }

    /// Gets the default soundboard sounds.
    pub fn default_soundboard_sounds(&self) -> GetDefaultSoundboardSounds {
        GetDefaultSoundboardSounds {
            client: self.clone(),
        }
    }

    /// Gets the soundboard sounds of a guild.
    pub fn guild_soundboard_sounds(&self, guild_id: GuildId) -> GetGuildSoundboardSounds {
        GetGuildSoundboardSounds {
            client: self.clone(),
            guild_id,
        }
    }

    /// Gets one soundboard sound of a guild.
    pub fn guild_soundboard_sound(
        &self,
        guild_id: GuildId,
        sound_id: SoundboardSoundId,
    ) -> GetGuildSoundboardSound {
        GetGuildSoundboardSound {
            client: self.clone(),
            guild_id,
            sound_id,
        }
    }

    /// Adds a soundboard sound to a guild. The name has 2 to 32 characters.
    pub fn create_guild_soundboard_sound(
        &self,
        guild_id: GuildId,
        name: impl Into<String>,
        sound: SoundData,
    ) -> CreateGuildSoundboardSound {
        CreateGuildSoundboardSound {
            client: self.clone(),
            guild_id,
            fields: CreateSoundBody {
                name: name.into(),
                sound,
                volume: None,
                emoji_id: None,
                emoji_name: None,
            },
            reason: None,
        }
    }

    /// Changes a soundboard sound of a guild.
    pub fn modify_guild_soundboard_sound(
        &self,
        guild_id: GuildId,
        sound_id: SoundboardSoundId,
    ) -> ModifyGuildSoundboardSound {
        ModifyGuildSoundboardSound {
            client: self.clone(),
            guild_id,
            sound_id,
            fields: ModifySoundBody::default(),
            reason: None,
        }
    }

    /// Deletes a soundboard sound of a guild.
    pub fn delete_guild_soundboard_sound(
        &self,
        guild_id: GuildId,
        sound_id: SoundboardSoundId,
    ) -> DeleteGuildSoundboardSound {
        DeleteGuildSoundboardSound {
            client: self.clone(),
            guild_id,
            sound_id,
            reason: None,
        }
    }
}
