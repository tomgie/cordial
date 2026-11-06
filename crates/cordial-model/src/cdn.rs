//! URLs of images and other assets on the Discord CDN.
//!
//! ```
//! use cordial_model::ImageHash;
//! use cordial_model::cdn::{self, ImageFormat};
//! use cordial_model::id::UserId;
//!
//! let hash = ImageHash::new("a_8342729096ea3675442027381ff50dfe");
//! let url = cdn::user_avatar(UserId::new_const(80351110224678912), &hash).size(256);
//! assert_eq!(
//!     url.to_string(),
//!     "https://cdn.discordapp.com/avatars/80351110224678912/a_8342729096ea3675442027381ff50dfe.webp?size=256&animated=true"
//! );
//! assert!(url.format(ImageFormat::Png).to_string().ends_with(".png?size=256"));
//! ```

use std::fmt;

use crate::ImageHash;
use crate::id::{
    ApplicationId, EmojiId, GenericId, GuildId, RoleId, ScheduledEventId, SoundboardSoundId,
    StickerId, TeamId, UserId,
};
use crate::sticker::StickerFormatType;

/// The base URL of the CDN.
pub const CDN_BASE: &str = "https://cdn.discordapp.com";

/// The application ID of the sticker pack banners.
const STICKER_PACK_APPLICATION: u64 = 710_982_414_301_790_216;

/// The file format of an image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ImageFormat {
    /// PNG.
    Png,
    /// JPEG.
    Jpeg,
    /// WebP. Animated assets stay animated.
    WebP,
    /// GIF. Only for animated assets.
    Gif,
    /// Lottie (JSON). Only for Lottie stickers.
    Lottie,
}

impl ImageFormat {
    /// Gives the file extension.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
            Self::Gif => "gif",
            Self::Lottie => "json",
        }
    }
}

/// A URL of an image on the CDN, with a format and a size.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ImageUrl {
    base: &'static str,
    path: String,
    format: ImageFormat,
    animated: bool,
    size: Option<u16>,
}

impl ImageUrl {
    fn new(path: String, animated: bool) -> Self {
        let format = if animated {
            ImageFormat::WebP
        } else {
            ImageFormat::Png
        };
        Self {
            base: CDN_BASE,
            path,
            format,
            animated,
            size: None,
        }
    }

    fn for_hash(path: &str, hash: &ImageHash) -> Self {
        Self::new(format!("{path}/{hash}"), hash.is_animated())
    }

    /// Sets the file format. The default is WebP for animated assets and PNG for others.
    #[must_use]
    pub fn format(mut self, format: ImageFormat) -> Self {
        self.format = format;
        self
    }

    /// Sets the size: a power of two from 16 to 4096. Other values round up to a valid size.
    #[must_use]
    pub fn size(mut self, size: u16) -> Self {
        self.size = Some(size.clamp(16, 4096).next_power_of_two());
        self
    }
}

impl fmt::Display for ImageUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}/{}.{}",
            self.base,
            self.path,
            self.format.extension()
        )?;

        let mut separator = '?';
        if let Some(size) = self.size {
            write!(formatter, "{separator}size={size}")?;
            separator = '&';
        }
        if self.animated && self.format == ImageFormat::WebP {
            write!(formatter, "{separator}animated=true")?;
        }
        Ok(())
    }
}

impl From<ImageUrl> for String {
    fn from(url: ImageUrl) -> Self {
        url.to_string()
    }
}

/// The avatar of a user.
#[must_use]
pub fn user_avatar(user_id: UserId, hash: &ImageHash) -> ImageUrl {
    ImageUrl::for_hash(&format!("avatars/{user_id}"), hash)
}

/// The default avatar of a user. See [`crate::user::User::default_avatar_index`].
#[must_use]
pub fn default_avatar(index: u8) -> ImageUrl {
    ImageUrl::new(format!("embed/avatars/{index}"), false)
}

/// The banner of a user.
#[must_use]
pub fn user_banner(user_id: UserId, hash: &ImageHash) -> ImageUrl {
    ImageUrl::for_hash(&format!("banners/{user_id}"), hash)
}

/// The avatar of a member in a guild.
#[must_use]
pub fn member_avatar(guild_id: GuildId, user_id: UserId, hash: &ImageHash) -> ImageUrl {
    ImageUrl::for_hash(&format!("guilds/{guild_id}/users/{user_id}/avatars"), hash)
}

/// The banner of a member in a guild.
#[must_use]
pub fn member_banner(guild_id: GuildId, user_id: UserId, hash: &ImageHash) -> ImageUrl {
    ImageUrl::for_hash(&format!("guilds/{guild_id}/users/{user_id}/banners"), hash)
}

/// An avatar decoration. Always PNG.
#[must_use]
pub fn avatar_decoration(asset: &ImageHash) -> ImageUrl {
    ImageUrl::new(format!("avatar-decoration-presets/{asset}"), false)
}

/// The icon of a guild.
#[must_use]
pub fn guild_icon(guild_id: GuildId, hash: &ImageHash) -> ImageUrl {
    ImageUrl::for_hash(&format!("icons/{guild_id}"), hash)
}

/// The splash of a guild.
#[must_use]
pub fn guild_splash(guild_id: GuildId, hash: &ImageHash) -> ImageUrl {
    ImageUrl::for_hash(&format!("splashes/{guild_id}"), hash)
}

/// The discovery splash of a guild.
#[must_use]
pub fn guild_discovery_splash(guild_id: GuildId, hash: &ImageHash) -> ImageUrl {
    ImageUrl::for_hash(&format!("discovery-splashes/{guild_id}"), hash)
}

/// The banner of a guild.
#[must_use]
pub fn guild_banner(guild_id: GuildId, hash: &ImageHash) -> ImageUrl {
    ImageUrl::for_hash(&format!("banners/{guild_id}"), hash)
}

/// The badge of a guild tag.
#[must_use]
pub fn guild_tag_badge(guild_id: GuildId, hash: &ImageHash) -> ImageUrl {
    ImageUrl::for_hash(&format!("guild-tag-badges/{guild_id}"), hash)
}

/// A custom emoji.
#[must_use]
pub fn emoji(emoji_id: EmojiId, animated: bool) -> ImageUrl {
    ImageUrl::new(format!("emojis/{emoji_id}"), animated)
}

/// The icon of a role.
#[must_use]
pub fn role_icon(role_id: RoleId, hash: &ImageHash) -> ImageUrl {
    ImageUrl::for_hash(&format!("role-icons/{role_id}"), hash)
}

/// The cover of a scheduled event.
#[must_use]
pub fn scheduled_event_cover(event_id: ScheduledEventId, hash: &ImageHash) -> ImageUrl {
    ImageUrl::for_hash(&format!("guild-events/{event_id}"), hash)
}

/// The icon of an application.
#[must_use]
pub fn application_icon(application_id: ApplicationId, hash: &ImageHash) -> ImageUrl {
    ImageUrl::for_hash(&format!("app-icons/{application_id}"), hash)
}

/// The cover image of an application.
#[must_use]
pub fn application_cover(application_id: ApplicationId, hash: &ImageHash) -> ImageUrl {
    application_icon(application_id, hash)
}

/// An asset of an application, for example a rich presence image.
#[must_use]
pub fn application_asset(application_id: ApplicationId, asset_id: GenericId) -> ImageUrl {
    ImageUrl::new(format!("app-assets/{application_id}/{asset_id}"), false)
}

/// The icon of an achievement.
#[must_use]
pub fn achievement_icon(
    application_id: ApplicationId,
    achievement_id: GenericId,
    hash: &ImageHash,
) -> ImageUrl {
    ImageUrl::for_hash(
        &format!("app-assets/{application_id}/achievements/{achievement_id}/icons"),
        hash,
    )
}

/// An asset of a store page.
#[must_use]
pub fn store_page_asset(application_id: ApplicationId, asset_id: GenericId) -> ImageUrl {
    ImageUrl::new(
        format!("app-assets/{application_id}/store/{asset_id}"),
        false,
    )
}

/// The banner of a sticker pack.
#[must_use]
pub fn sticker_pack_banner(banner_asset_id: GenericId) -> ImageUrl {
    ImageUrl::new(
        format!("app-assets/{STICKER_PACK_APPLICATION}/store/{banner_asset_id}"),
        false,
    )
}

/// The icon of a team.
#[must_use]
pub fn team_icon(team_id: TeamId, hash: &ImageHash) -> ImageUrl {
    ImageUrl::for_hash(&format!("team-icons/{team_id}"), hash)
}

/// A sticker. The format follows the format type: GIF stickers come from `media.discordapp.net`.
#[must_use]
pub fn sticker(sticker_id: StickerId, format_type: StickerFormatType) -> ImageUrl {
    let mut url = ImageUrl::new(format!("stickers/{sticker_id}"), false);
    url.format = match format_type {
        StickerFormatType::Lottie => ImageFormat::Lottie,
        StickerFormatType::Gif => {
            url.base = "https://media.discordapp.net";
            ImageFormat::Gif
        }
        _ => ImageFormat::Png,
    };
    url
}

/// The URL of a soundboard sound file (MP3 or Ogg).
#[must_use]
pub fn soundboard_sound(sound_id: SoundboardSoundId) -> String {
    format!("{CDN_BASE}/soundboard-sounds/{sound_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_hash_defaults_to_png() {
        let hash = ImageHash::new("8342729096ea3675442027381ff50dfe");

        let url = guild_icon(GuildId::new_const(1), &hash).to_string();

        assert_eq!(
            url,
            "https://cdn.discordapp.com/icons/1/8342729096ea3675442027381ff50dfe.png"
        );
    }

    #[test]
    fn rounds_size_to_power_of_two() {
        let url = default_avatar(3).size(100).to_string();

        assert_eq!(
            url,
            "https://cdn.discordapp.com/embed/avatars/3.png?size=128"
        );
        assert!(
            default_avatar(0)
                .size(9000)
                .to_string()
                .ends_with("size=4096")
        );
    }

    #[test]
    fn gif_stickers_use_media_host() {
        let url = sticker(StickerId::new_const(7), StickerFormatType::Gif).to_string();

        assert_eq!(url, "https://media.discordapp.net/stickers/7.gif");
    }
}
