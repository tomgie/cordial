//! Stickers and sticker packs.

use serde::{Deserialize, Serialize};

use crate::id::{GuildId, SkuId, StickerId, StickerPackId};
use crate::user::User;

/// A sticker that can be sent in messages.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Sticker {
    /// The sticker ID.
    pub id: StickerId,
    /// For a standard sticker, the ID of its pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack_id: Option<StickerPackId>,
    /// The name.
    pub name: String,
    /// The description.
    #[serde(default)]
    pub description: Option<String>,
    /// Autocomplete and suggestion tags, up to 200 characters.
    #[serde(default)]
    pub tags: String,
    /// The sticker type.
    #[serde(rename = "type")]
    pub kind: StickerType,
    /// The file format.
    pub format_type: StickerFormatType,
    /// Tells if the guild sticker can be used. It can be `false` after the guild loses Server Boosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available: Option<bool>,
    /// The ID of the guild that owns the sticker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The user that uploaded the guild sticker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// The sort order of a standard sticker in its pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_value: Option<u32>,
}

crate::discord_enum! {
    /// The type of a sticker.
    pub enum StickerType: u8 {
        /// An official sticker in a pack.
        Standard = 1,
        /// A sticker that a guild uploaded.
        Guild = 2,
    }
}

crate::discord_enum! {
    /// The file format of a sticker.
    pub enum StickerFormatType: u8 {
        /// PNG.
        Png = 1,
        /// Animated PNG.
        Apng = 2,
        /// Lottie (JSON).
        Lottie = 3,
        /// GIF.
        Gif = 4,
    }
}

impl StickerFormatType {
    /// Gives the file extension on the CDN.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Lottie => "json",
            Self::Gif => "gif",
            Self::Png | Self::Apng | Self::Unknown(_) => "png",
        }
    }
}

/// The smallest data to show a sticker.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct StickerItem {
    /// The sticker ID.
    pub id: StickerId,
    /// The name.
    pub name: String,
    /// The file format.
    pub format_type: StickerFormatType,
}

/// A pack of standard stickers.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct StickerPack {
    /// The pack ID.
    pub id: StickerPackId,
    /// The stickers in the pack.
    pub stickers: Vec<Sticker>,
    /// The name.
    pub name: String,
    /// The ID of the SKU of the pack.
    pub sku_id: SkuId,
    /// The ID of the sticker that is the icon of the pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover_sticker_id: Option<StickerId>,
    /// The description.
    pub description: String,
    /// The ID of the banner image of the pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner_asset_id: Option<crate::id::GenericId>,
}

/// The response of `GET /sticker-packs`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct StickerPacks {
    /// The sticker packs.
    pub sticker_packs: Vec<StickerPack>,
}
