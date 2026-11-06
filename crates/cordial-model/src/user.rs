//! Users, connections, and application role connections.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::guild::Integration;
use crate::id::{GuildId, SkuId, UserId};
use crate::{Color, ImageHash, Locale};

/// A Discord user.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct User {
    /// The user ID.
    pub id: UserId,
    /// The username. It is not unique across the platform.
    pub username: String,
    /// The legacy Discord tag. It is `"0"` for users with a new username.
    pub discriminator: String,
    /// The display name, if it is set.
    #[serde(default)]
    pub global_name: Option<String>,
    /// The avatar hash.
    #[serde(default)]
    pub avatar: Option<ImageHash>,
    /// Tells if the user belongs to an OAuth2 application.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub bot: bool,
    /// Tells if the user is an official Discord system user.
    #[serde(default, skip_serializing_if = "crate::serde_util::is_false")]
    pub system: bool,
    /// Tells if the user has two-factor authentication on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mfa_enabled: Option<bool>,
    /// The banner hash.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner: Option<ImageHash>,
    /// The banner color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accent_color: Option<Color>,
    /// The chosen language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<Locale>,
    /// Tells if the email is verified. Needs the `email` scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified: Option<bool>,
    /// The email address. Needs the `email` scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// The flags on the account.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<UserFlags>,
    /// The Nitro subscription type. Needs the `identify.premium` scope, else it is `None` (0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premium_type: Option<PremiumType>,
    /// The public flags on the account.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_flags: Option<UserFlags>,
    /// The avatar decoration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
    /// The collectibles, for example a nameplate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collectibles: Option<Collectibles>,
    /// The primary guild (the guild tag that the user shows).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_guild: Option<PrimaryGuild>,
}

impl User {
    /// Gives the name to show: the display name, or else the username.
    #[must_use]
    pub fn display_name(&self) -> &str {
        self.global_name.as_deref().unwrap_or(&self.username)
    }

    /// Gives the mention text, for example `<@80351110224678912>`.
    #[must_use]
    pub fn mention(&self) -> String {
        format!("<@{}>", self.id)
    }

    /// Tells if the user has the legacy tag system (a discriminator other than `"0"`).
    #[must_use]
    pub fn has_legacy_tag(&self) -> bool {
        self.discriminator != "0"
    }

    /// Gives the index of the default avatar, for the CDN path `embed/avatars/{index}.png`.
    #[must_use]
    pub fn default_avatar_index(&self) -> u8 {
        #[allow(clippy::cast_possible_truncation)]
        match self.discriminator.parse::<u16>() {
            Ok(discriminator) if discriminator != 0 => (discriminator % 5) as u8,
            _ => ((self.id.get() >> 22) % 6) as u8,
        }
    }

    /// Gives the URL of the avatar, or of the default avatar when the user has none.
    #[must_use]
    pub fn avatar_url(&self) -> crate::cdn::ImageUrl {
        match &self.avatar {
            Some(hash) => crate::cdn::user_avatar(self.id, hash),
            None => crate::cdn::default_avatar(self.default_avatar_index()),
        }
    }
}

crate::discord_flags! {
    /// Flags on a user account.
    pub struct UserFlags: u64 as integer {
        /// Discord employee.
        const STAFF = 1 << 0;
        /// Owner of a partnered server.
        const PARTNER = 1 << 1;
        /// HypeSquad Events member.
        const HYPESQUAD = 1 << 2;
        /// Bug Hunter level 1.
        const BUG_HUNTER_LEVEL_1 = 1 << 3;
        /// House Bravery member.
        const HYPESQUAD_ONLINE_HOUSE_1 = 1 << 6;
        /// House Brilliance member.
        const HYPESQUAD_ONLINE_HOUSE_2 = 1 << 7;
        /// House Balance member.
        const HYPESQUAD_ONLINE_HOUSE_3 = 1 << 8;
        /// Early Nitro supporter.
        const PREMIUM_EARLY_SUPPORTER = 1 << 9;
        /// The user is a team.
        const TEAM_PSEUDO_USER = 1 << 10;
        /// Bug Hunter level 2.
        const BUG_HUNTER_LEVEL_2 = 1 << 14;
        /// Verified bot.
        const VERIFIED_BOT = 1 << 16;
        /// Early verified bot developer.
        const VERIFIED_DEVELOPER = 1 << 17;
        /// Moderator Programs alumni.
        const CERTIFIED_MODERATOR = 1 << 18;
        /// The bot uses only HTTP interactions and shows in the online member list.
        const BOT_HTTP_INTERACTIONS = 1 << 19;
        /// Active developer. The docs no longer list this flag (2026-03), but Discord can still send it.
        const ACTIVE_DEVELOPER = 1 << 22;
    }
}

crate::discord_enum! {
    /// The type of Nitro subscription of a user.
    pub enum PremiumType: u8 {
        /// No Nitro.
        None = 0,
        /// Nitro Classic.
        NitroClassic = 1,
        /// Nitro.
        Nitro = 2,
        /// Nitro Basic.
        NitroBasic = 3,
    }
}

/// The avatar decoration of a user or a member.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AvatarDecorationData {
    /// The avatar decoration hash.
    pub asset: ImageHash,
    /// The ID of the SKU of the avatar decoration.
    pub sku_id: SkuId,
}

/// The collectibles of a user, other than avatar decorations and profile effects.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Collectibles {
    /// The nameplate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nameplate: Option<Nameplate>,
}

/// The nameplate of a user.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Nameplate {
    /// The ID of the nameplate SKU.
    pub sku_id: SkuId,
    /// The path of the nameplate asset.
    pub asset: String,
    /// The label. Discord does not use it now.
    pub label: String,
    /// The background color.
    pub palette: NameplatePalette,
}

crate::discord_string_enum! {
    /// The background color of a nameplate.
    pub enum NameplatePalette {
        /// Crimson.
        Crimson = "crimson",
        /// Berry.
        Berry = "berry",
        /// Sky.
        Sky = "sky",
        /// Teal.
        Teal = "teal",
        /// Forest.
        Forest = "forest",
        /// Bubble gum.
        BubbleGum = "bubble_gum",
        /// Violet.
        Violet = "violet",
        /// Cobalt.
        Cobalt = "cobalt",
        /// Clover.
        Clover = "clover",
        /// Lemon.
        Lemon = "lemon",
        /// White.
        White = "white",
    }
}

/// The primary guild of a user: the guild whose tag the user shows.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PrimaryGuild {
    /// The ID of the primary guild.
    #[serde(default)]
    pub identity_guild_id: Option<GuildId>,
    /// Tells if the user shows the tag. `None` means that the system cleared it.
    /// `Some(false)` means that the user removed it.
    #[serde(default)]
    pub identity_enabled: Option<bool>,
    /// The text of the tag, up to 4 characters.
    #[serde(default)]
    pub tag: Option<String>,
    /// The hash of the tag badge.
    #[serde(default)]
    pub badge: Option<ImageHash>,
}

/// An account on another service that the user connected.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Connection {
    /// The ID of the account on the service.
    pub id: String,
    /// The username on the service.
    pub name: String,
    /// The service.
    #[serde(rename = "type")]
    pub kind: ConnectionService,
    /// Tells if the connection is revoked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked: Option<bool>,
    /// Partial guild integrations of the connection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrations: Option<Vec<Integration>>,
    /// Tells if the connection is verified.
    pub verified: bool,
    /// Tells if friend sync is on.
    pub friend_sync: bool,
    /// Tells if activities of this connection show in presence updates.
    pub show_activity: bool,
    /// Tells if the connection has a third-party OAuth2 token.
    pub two_way_link: bool,
    /// Who can see the connection.
    pub visibility: ConnectionVisibility,
}

crate::discord_string_enum! {
    /// A service that a user can connect.
    pub enum ConnectionService {
        /// Amazon Music.
        AmazonMusic = "amazon-music",
        /// Battle.net. Discord no longer returns it (2026-09).
        BattleNet = "battlenet",
        /// Bungie.net.
        Bungie = "bungie",
        /// Bluesky.
        Bluesky = "bluesky",
        /// Crunchyroll.
        Crunchyroll = "crunchyroll",
        /// A domain.
        Domain = "domain",
        /// eBay.
        Ebay = "ebay",
        /// Epic Games.
        EpicGames = "epicgames",
        /// Facebook.
        Facebook = "facebook",
        /// GitHub.
        GitHub = "github",
        /// Instagram. Users can no longer add it.
        Instagram = "instagram",
        /// League of Legends. Discord no longer returns it (2026-07).
        LeagueOfLegends = "leagueoflegends",
        /// Mastodon.
        Mastodon = "mastodon",
        /// PayPal.
        PayPal = "paypal",
        /// PlayStation Network.
        PlayStation = "playstation",
        /// Reddit.
        Reddit = "reddit",
        /// Riot Games. Discord no longer returns it (2026-07).
        RiotGames = "riotgames",
        /// Roblox.
        Roblox = "roblox",
        /// Spotify.
        Spotify = "spotify",
        /// Skype. Users can no longer add it.
        Skype = "skype",
        /// Steam.
        Steam = "steam",
        /// TikTok.
        TikTok = "tiktok",
        /// Twitch.
        Twitch = "twitch",
        /// X (Twitter).
        Twitter = "twitter",
        /// Xbox.
        Xbox = "xbox",
        /// YouTube.
        YouTube = "youtube",
    }
}

crate::discord_enum! {
    /// Who can see a connection.
    pub enum ConnectionVisibility: u8 {
        /// Only the user.
        None = 0,
        /// Everyone.
        Everyone = 1,
    }
}

/// The role connection that an application attached to a user.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ApplicationRoleConnection {
    /// The name of the platform, up to 50 characters.
    #[serde(default)]
    pub platform_name: Option<String>,
    /// The username on the platform, up to 100 characters.
    #[serde(default)]
    pub platform_username: Option<String>,
    /// The metadata values, keyed by the metadata keys. Each value is a string of up to 100 characters.
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}
