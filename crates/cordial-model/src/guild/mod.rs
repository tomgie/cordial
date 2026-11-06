//! Guilds and their parts: roles, members, integrations, onboarding, and more.

mod integration;
mod member;
mod role;

pub use integration::{
    Integration, IntegrationAccount, IntegrationApplication, IntegrationExpireBehavior,
    IntegrationType,
};
pub use member::{Member, MemberFlags, PartialMember};
pub use role::{PartialRole, Role, RoleColors, RoleFlags, RoleTags};

use serde::{Deserialize, Serialize};

use crate::emoji::Emoji;
use crate::id::{ApplicationId, ChannelId, EmojiId, GuildId, OnboardingPromptId, RoleId, UserId};
use crate::sticker::Sticker;
use crate::user::User;
use crate::{ImageHash, Locale, Permissions, Timestamp};

/// A guild (a "server" in the Discord client).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Guild {
    /// The guild ID.
    pub id: GuildId,
    /// The name, 2 to 100 characters.
    pub name: String,
    /// The icon hash.
    #[serde(default)]
    pub icon: Option<ImageHash>,
    /// The icon hash, in the template object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_hash: Option<ImageHash>,
    /// The splash hash.
    #[serde(default)]
    pub splash: Option<ImageHash>,
    /// The discovery splash hash. Only for guilds with the `DISCOVERABLE` feature.
    #[serde(default)]
    pub discovery_splash: Option<ImageHash>,
    /// Tells if the current user owns the guild. Only from `GET /users/@me/guilds`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<bool>,
    /// The ID of the owner.
    pub owner_id: UserId,
    /// The permissions of the current user, without overwrites. Only from `GET /users/@me/guilds`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,
    /// The voice region. Deprecated: use the `rtc_region` of channels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// The AFK channel.
    #[serde(default)]
    pub afk_channel_id: Option<ChannelId>,
    /// The AFK timeout in seconds.
    #[serde(default)]
    pub afk_timeout: u32,
    /// Tells if the widget is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget_enabled: Option<bool>,
    /// The channel that the widget makes invites to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget_channel_id: Option<ChannelId>,
    /// The verification level for members.
    #[serde(default)]
    pub verification_level: VerificationLevel,
    /// The default notification level.
    #[serde(default)]
    pub default_message_notifications: DefaultMessageNotificationLevel,
    /// The explicit content filter level.
    #[serde(default)]
    pub explicit_content_filter: ExplicitContentFilter,
    /// The roles.
    #[serde(default)]
    pub roles: Vec<Role>,
    /// The custom emojis.
    #[serde(default)]
    pub emojis: Vec<Emoji>,
    /// The features.
    #[serde(default)]
    pub features: Vec<GuildFeature>,
    /// The MFA level for moderation actions.
    #[serde(default)]
    pub mfa_level: MfaLevel,
    /// The application that made the guild, if a bot made it.
    #[serde(default)]
    pub application_id: Option<ApplicationId>,
    /// The channel for system messages, for example welcome messages.
    #[serde(default)]
    pub system_channel_id: Option<ChannelId>,
    /// The flags of the system channel.
    #[serde(default)]
    pub system_channel_flags: SystemChannelFlags,
    /// The rules channel of a Community guild.
    #[serde(default)]
    pub rules_channel_id: Option<ChannelId>,
    /// The maximum number of presences. Almost always `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_presences: Option<u64>,
    /// The maximum number of members.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_members: Option<u64>,
    /// The vanity URL code.
    #[serde(default)]
    pub vanity_url_code: Option<String>,
    /// The description.
    #[serde(default)]
    pub description: Option<String>,
    /// The banner hash.
    #[serde(default)]
    pub banner: Option<ImageHash>,
    /// The Server Boost level.
    #[serde(default)]
    pub premium_tier: PremiumTier,
    /// The number of boosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premium_subscription_count: Option<u32>,
    /// The preferred locale of a Community guild.
    #[serde(default)]
    pub preferred_locale: Locale,
    /// The channel for notices from Discord to moderators of a Community guild.
    #[serde(default)]
    pub public_updates_channel_id: Option<ChannelId>,
    /// The maximum number of users in a video channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_video_channel_users: Option<u32>,
    /// The maximum number of users in a stage video channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_stage_video_channel_users: Option<u32>,
    /// The approximate number of members. Only when the request asks for counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate_member_count: Option<u64>,
    /// The approximate number of members that are online. Only when the request asks for counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate_presence_count: Option<u64>,
    /// The welcome screen of a Community guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub welcome_screen: Option<WelcomeScreen>,
    /// The age-restriction level.
    #[serde(default)]
    pub nsfw_level: NsfwLevel,
    /// The custom stickers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stickers: Option<Vec<Sticker>>,
    /// Tells if the boost progress bar is on.
    #[serde(default)]
    pub premium_progress_bar_enabled: bool,
    /// The channel for safety alerts to moderators of a Community guild.
    #[serde(default)]
    pub safety_alerts_channel_id: Option<ChannelId>,
    /// The incidents data.
    #[serde(default)]
    pub incidents_data: Option<IncidentsData>,
}

impl Guild {
    /// Tells if the guild has a feature.
    #[must_use]
    pub fn has_feature(&self, feature: &GuildFeature) -> bool {
        self.features.contains(feature)
    }

    /// Gives the `@everyone` role. Its ID is the guild ID.
    #[must_use]
    pub fn everyone_role(&self) -> Option<&Role> {
        let everyone_id: RoleId = self.id.cast();
        self.roles.iter().find(|role| role.id == everyone_id)
    }
}

/// A partial guild, as in `GET /users/@me/guilds`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct CurrentUserGuild {
    /// The guild ID.
    pub id: GuildId,
    /// The name.
    pub name: String,
    /// The icon hash.
    #[serde(default)]
    pub icon: Option<ImageHash>,
    /// The banner hash.
    #[serde(default)]
    pub banner: Option<ImageHash>,
    /// Tells if the current user owns the guild.
    #[serde(default)]
    pub owner: bool,
    /// The permissions of the current user, without overwrites.
    pub permissions: Permissions,
    /// The features.
    #[serde(default)]
    pub features: Vec<GuildFeature>,
    /// The approximate number of members. Only when the request asks for counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate_member_count: Option<u64>,
    /// The approximate number of online members. Only when the request asks for counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approximate_presence_count: Option<u64>,
}

/// A guild that is not available, as in `READY` and in outages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct UnavailableGuild {
    /// The guild ID.
    pub id: GuildId,
    /// Tells if the guild is not available. In `GUILD_DELETE`, `true` means an outage and
    /// `false` (absent) means that the bot left or was removed.
    #[serde(default)]
    pub unavailable: bool,
}

crate::discord_enum! {
    /// The default notification level of a guild.
    pub enum DefaultMessageNotificationLevel: u8 {
        /// Members get notifications for all messages.
        AllMessages = 0,
        /// Members get notifications only for messages that mention them.
        OnlyMentions = 1,
    }
}

crate::discord_enum! {
    /// The explicit content filter level of a guild.
    pub enum ExplicitContentFilter: u8 {
        /// No scan of media content.
        Disabled = 0,
        /// Scan media content from members without roles.
        MembersWithoutRoles = 1,
        /// Scan media content from all members.
        AllMembers = 2,
    }
}

crate::discord_enum! {
    /// The MFA level of a guild for moderation actions.
    pub enum MfaLevel: u8 {
        /// No MFA requirement.
        None = 0,
        /// MFA is required for moderation actions.
        Elevated = 1,
    }
}

crate::discord_enum! {
    /// The verification level of a guild.
    pub enum VerificationLevel: u8 {
        /// No restriction.
        None = 0,
        /// A verified email is required.
        Low = 1,
        /// Registered on Discord for more than 5 minutes.
        Medium = 2,
        /// Member of the guild for more than 10 minutes.
        High = 3,
        /// A verified phone number is required.
        VeryHigh = 4,
    }
}

crate::discord_enum! {
    /// The age-restriction level of a guild.
    pub enum NsfwLevel: u8 {
        /// Default.
        Default = 0,
        /// Explicit.
        Explicit = 1,
        /// Safe.
        Safe = 2,
        /// Age-restricted.
        AgeRestricted = 3,
    }
}

crate::discord_enum! {
    /// The Server Boost level of a guild.
    pub enum PremiumTier: u8 {
        /// No boost level.
        None = 0,
        /// Level 1.
        Tier1 = 1,
        /// Level 2.
        Tier2 = 2,
        /// Level 3.
        Tier3 = 3,
    }
}

crate::discord_flags! {
    /// Flags of the system channel of a guild.
    pub struct SystemChannelFlags: u64 as integer {
        /// Suppress member join notifications.
        const SUPPRESS_JOIN_NOTIFICATIONS = 1 << 0;
        /// Suppress Server Boost notifications.
        const SUPPRESS_PREMIUM_SUBSCRIPTIONS = 1 << 1;
        /// Suppress server setup tips.
        const SUPPRESS_GUILD_REMINDER_NOTIFICATIONS = 1 << 2;
        /// Hide the sticker reply buttons on member join notifications.
        const SUPPRESS_JOIN_NOTIFICATION_REPLIES = 1 << 3;
        /// Suppress role subscription purchase and renewal notifications.
        const SUPPRESS_ROLE_SUBSCRIPTION_PURCHASE_NOTIFICATIONS = 1 << 4;
        /// Hide the sticker reply buttons on role subscription notifications.
        const SUPPRESS_ROLE_SUBSCRIPTION_PURCHASE_NOTIFICATION_REPLIES = 1 << 5;
    }
}

crate::discord_string_enum! {
    /// A feature of a guild.
    pub enum GuildFeature {
        /// The guild can set an animated banner.
        AnimatedBanner = "ANIMATED_BANNER",
        /// The guild can set an animated icon.
        AnimatedIcon = "ANIMATED_ICON",
        /// The guild uses the old command permissions behavior.
        ApplicationCommandPermissionsV2 = "APPLICATION_COMMAND_PERMISSIONS_V2",
        /// The guild has auto moderation rules.
        AutoModeration = "AUTO_MODERATION",
        /// The guild can set a banner.
        Banner = "BANNER",
        /// The guild is a Community guild.
        Community = "COMMUNITY",
        /// The guild has monetization.
        CreatorMonetizableProvisional = "CREATOR_MONETIZABLE_PROVISIONAL",
        /// The guild has the role subscription promo page.
        CreatorStorePage = "CREATOR_STORE_PAGE",
        /// The guild is a support server in the App Directory.
        DeveloperSupportServer = "DEVELOPER_SUPPORT_SERVER",
        /// The guild can be discovered in the directory.
        Discoverable = "DISCOVERABLE",
        /// The guild can set gradient role colors.
        EnhancedRoleColors = "ENHANCED_ROLE_COLORS",
        /// The guild can be featured in the directory.
        Featurable = "FEATURABLE",
        /// The guild can set guild tags.
        GuildTags = "GUILD_TAGS",
        /// The guild can use guest invites.
        GuestsEnabled = "GUESTS_ENABLED",
        /// The guild paused its invites.
        InvitesDisabled = "INVITES_DISABLED",
        /// The guild can set an invite splash.
        InviteSplash = "INVITE_SPLASH",
        /// The guild has Membership Screening.
        MemberVerificationGateEnabled = "MEMBER_VERIFICATION_GATE_ENABLED",
        /// The guild has more soundboard slots.
        MoreSoundboard = "MORE_SOUNDBOARD",
        /// The guild has more sticker slots.
        MoreStickers = "MORE_STICKERS",
        /// The guild can make announcement channels.
        News = "NEWS",
        /// The guild is partnered.
        Partnered = "PARTNERED",
        /// The guild can be previewed before joining.
        PreviewEnabled = "PREVIEW_ENABLED",
        /// Prune needs the ADMINISTRATOR permission.
        PruneRequiresAdmin = "PRUNE_REQUIRES_ADMIN",
        /// The guild turned off raid alerts.
        RaidAlertsDisabled = "RAID_ALERTS_DISABLED",
        /// The guild can set role icons.
        RoleIcons = "ROLE_ICONS",
        /// The guild has role subscriptions for purchase.
        RoleSubscriptionsAvailableForPurchase = "ROLE_SUBSCRIPTIONS_AVAILABLE_FOR_PURCHASE",
        /// The guild has role subscriptions.
        RoleSubscriptionsEnabled = "ROLE_SUBSCRIPTIONS_ENABLED",
        /// The guild has soundboard sounds.
        Soundboard = "SOUNDBOARD",
        /// The guild has ticketed events.
        TicketedEventsEnabled = "TICKETED_EVENTS_ENABLED",
        /// The guild can set a vanity URL.
        VanityUrl = "VANITY_URL",
        /// The guild is verified.
        Verified = "VERIFIED",
        /// The guild can set a voice bitrate of 384 kbps.
        VipRegions = "VIP_REGIONS",
        /// The guild has a welcome screen.
        WelcomeScreenEnabled = "WELCOME_SCREEN_ENABLED",
    }
}

/// The preview of a guild.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildPreview {
    /// The guild ID.
    pub id: GuildId,
    /// The name.
    pub name: String,
    /// The icon hash.
    #[serde(default)]
    pub icon: Option<ImageHash>,
    /// The splash hash.
    #[serde(default)]
    pub splash: Option<ImageHash>,
    /// The discovery splash hash.
    #[serde(default)]
    pub discovery_splash: Option<ImageHash>,
    /// The custom emojis.
    #[serde(default)]
    pub emojis: Vec<Emoji>,
    /// The features.
    #[serde(default)]
    pub features: Vec<GuildFeature>,
    /// The approximate number of members.
    pub approximate_member_count: u64,
    /// The approximate number of online members.
    pub approximate_presence_count: u64,
    /// The description.
    #[serde(default)]
    pub description: Option<String>,
    /// The custom stickers.
    #[serde(default)]
    pub stickers: Vec<Sticker>,
}

/// The widget settings of a guild.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildWidgetSettings {
    /// Tells if the widget is on.
    pub enabled: bool,
    /// The widget channel.
    #[serde(default)]
    pub channel_id: Option<ChannelId>,
}

/// The public widget of a guild.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildWidget {
    /// The guild ID.
    pub id: GuildId,
    /// The name.
    pub name: String,
    /// The invite for the widget channel.
    #[serde(default)]
    pub instant_invite: Option<String>,
    /// The voice and stage channels that `@everyone` can access.
    #[serde(default)]
    pub channels: Vec<GuildWidgetChannel>,
    /// The online members, up to 100. IDs, discriminators, and avatars are anonymized.
    #[serde(default)]
    pub members: Vec<GuildWidgetMember>,
    /// The number of online members.
    pub presence_count: u64,
}

/// A channel in a guild widget.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildWidgetChannel {
    /// The channel ID.
    pub id: ChannelId,
    /// The name.
    pub name: String,
    /// The position.
    pub position: i32,
}

/// A member in a guild widget. The ID, discriminator, and avatar are anonymized.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildWidgetMember {
    /// An anonymized ID. It is not a user ID.
    pub id: String,
    /// The username.
    pub username: String,
    /// An anonymized discriminator.
    #[serde(default)]
    pub discriminator: String,
    /// An anonymized avatar.
    #[serde(default)]
    pub avatar: Option<String>,
    /// The status, for example `online`.
    pub status: String,
    /// The URL of the avatar.
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// The current activity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity: Option<GuildWidgetActivity>,
}

/// The activity of a member in a guild widget.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildWidgetActivity {
    /// The activity name.
    pub name: String,
}

/// A ban in a guild.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Ban {
    /// The reason for the ban.
    #[serde(default)]
    pub reason: Option<String>,
    /// The banned user.
    pub user: User,
}

/// The result of a bulk ban.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct BulkBanResult {
    /// The users that were banned.
    pub banned_users: Vec<UserId>,
    /// The users that could not be banned.
    pub failed_users: Vec<UserId>,
}

/// The welcome screen of a Community guild.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct WelcomeScreen {
    /// The description of the guild in the welcome screen.
    #[serde(default)]
    pub description: Option<String>,
    /// The channels in the welcome screen, up to 5.
    #[serde(default)]
    pub welcome_channels: Vec<WelcomeScreenChannel>,
}

/// A channel in a welcome screen.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct WelcomeScreenChannel {
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The description of the channel.
    pub description: String,
    /// The ID of a custom emoji.
    #[serde(default)]
    pub emoji_id: Option<EmojiId>,
    /// The name of a custom emoji, or a Unicode emoji, or `None` for no emoji.
    #[serde(default)]
    pub emoji_name: Option<String>,
}

/// The onboarding flow of a guild.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Onboarding {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The prompts.
    #[serde(default)]
    pub prompts: Vec<OnboardingPrompt>,
    /// The channels that members join automatically.
    #[serde(default)]
    pub default_channel_ids: Vec<ChannelId>,
    /// Tells if onboarding is on.
    #[serde(default)]
    pub enabled: bool,
    /// The onboarding mode.
    #[serde(default)]
    pub mode: OnboardingMode,
}

/// A prompt in onboarding.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct OnboardingPrompt {
    /// The prompt ID.
    pub id: OnboardingPromptId,
    /// The prompt type.
    #[serde(default, rename = "type")]
    pub kind: OnboardingPromptType,
    /// The options.
    #[serde(default)]
    pub options: Vec<OnboardingPromptOption>,
    /// The title.
    pub title: String,
    /// Tells if users can select only one option.
    #[serde(default)]
    pub single_select: bool,
    /// Tells if the prompt is required.
    #[serde(default)]
    pub required: bool,
    /// Tells if the prompt is in the onboarding flow. Else it is only in "Channels & Roles".
    #[serde(default)]
    pub in_onboarding: bool,
}

/// An option of an onboarding prompt.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct OnboardingPromptOption {
    /// The option ID.
    pub id: OnboardingPromptId,
    /// The channels that a member joins with this option.
    #[serde(default)]
    pub channel_ids: Vec<ChannelId>,
    /// The roles that a member gets with this option.
    #[serde(default)]
    pub role_ids: Vec<RoleId>,
    /// The emoji. Responses use it. Requests use the three `emoji_*` fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<Emoji>,
    /// The emoji ID, for requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji_id: Option<EmojiId>,
    /// The emoji name, for requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji_name: Option<String>,
    /// Tells if the emoji is animated, for requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji_animated: Option<bool>,
    /// The title.
    pub title: String,
    /// The description.
    #[serde(default)]
    pub description: Option<String>,
}

crate::discord_enum! {
    /// The mode of onboarding.
    pub enum OnboardingMode: u8 {
        /// Only default channels count toward the constraints.
        Default = 0,
        /// Default channels and questions count toward the constraints.
        Advanced = 1,
    }
}

crate::discord_enum! {
    /// The type of an onboarding prompt.
    pub enum OnboardingPromptType: u8 {
        /// Multiple choice.
        MultipleChoice = 0,
        /// A dropdown.
        Dropdown = 1,
    }
}

/// The incidents data of a guild.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct IncidentsData {
    /// When invites become available again.
    #[serde(default)]
    pub invites_disabled_until: Option<Timestamp>,
    /// When direct messages become available again.
    #[serde(default)]
    pub dms_disabled_until: Option<Timestamp>,
    /// When Discord detected DM spam.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dm_spam_detected_at: Option<Timestamp>,
    /// When Discord detected a raid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raid_detected_at: Option<Timestamp>,
}

/// The vanity URL of a guild.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct VanityUrl {
    /// The code. `None` when the guild has no vanity URL.
    #[serde(default)]
    pub code: Option<String>,
    /// The number of uses.
    #[serde(default)]
    pub uses: u64,
}

/// The result of a prune count or a prune.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PruneCount {
    /// The number of members. `None` when the request did not ask for the count.
    #[serde(default)]
    pub pruned: Option<u64>,
}
