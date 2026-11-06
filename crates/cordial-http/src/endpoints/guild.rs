//! Guild endpoints: the guild, its channels, threads, prune, regions, invites, integrations,
//! and incident actions.

use cordial_model::channel::{
    AutoArchiveDuration, Channel, ChannelFlags, ChannelType, DefaultReaction, ForumLayout,
    ForumTag, SortOrder, ThreadList, VideoQualityMode,
};
use cordial_model::guild::{
    DefaultMessageNotificationLevel, ExplicitContentFilter, Guild, GuildFeature, GuildPreview,
    IncidentsData, Integration, PruneCount, SystemChannelFlags, VanityUrl, VerificationLevel,
};
use cordial_model::id::{ChannelId, GuildId, IntegrationId, RoleId};
use cordial_model::invite::Invite;
use cordial_model::permissions::PermissionOverwrite;
use cordial_model::validate::ValidationError;
use cordial_model::voice::VoiceRegion;
use cordial_model::{Locale, Patch, Permissions, Timestamp};
use serde::{Deserialize, Serialize};

use super::limits;
use crate::client::Client;
use crate::error::Error;
use crate::request::{Endpoint, ImageData, RequestBody, check};
use crate::route::{MajorParameter, Method, Query, Route};

/// The AFK timeouts that Discord accepts, in seconds.
const AFK_TIMEOUTS: [u32; 5] = [60, 300, 900, 1800, 3600];
/// The maximum slow mode delay, in seconds.
const RATE_LIMIT_PER_USER_MAX: u64 = 21_600;

/// The computed permissions of a member or a role in a guild.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ComputedPermissions {
    /// The permissions.
    pub permissions: Permissions,
}

/// `GET /guilds/{guild.id}`: a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuild {
    client: Client,
    guild_id: GuildId,
    with_counts: Option<bool>,
}

impl GetGuild {
    /// Adds the approximate member and presence counts when `true`.
    pub fn with_counts(mut self, with_counts: bool) -> Self {
        self.with_counts = Some(with_counts);
        self
    }
}

impl Endpoint for GetGuild {
    type Response = Guild;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("with_counts", self.with_counts);
        Route::new(
            Method::Get,
            "/guilds/{guild_id}",
            format!("/guilds/{}{}", self.guild_id, query.finish()),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/preview`: the preview of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildPreview {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildPreview {
    type Response = GuildPreview;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/preview",
            format!("/guilds/{}/preview", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `PATCH /guilds/{guild.id}`: changes the settings of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuild {
    client: Client,
    guild_id: GuildId,
    body: EditGuildBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditGuildBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    region: Patch<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verification_level: Option<VerificationLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_message_notifications: Option<DefaultMessageNotificationLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    explicit_content_filter: Option<ExplicitContentFilter>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    afk_channel_id: Patch<ChannelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    afk_timeout: Option<u32>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    icon: Patch<ImageData>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    splash: Patch<ImageData>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    discovery_splash: Patch<ImageData>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    banner: Patch<ImageData>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    system_channel_id: Patch<ChannelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_channel_flags: Option<SystemChannelFlags>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    rules_channel_id: Patch<ChannelId>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    public_updates_channel_id: Patch<ChannelId>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    preferred_locale: Patch<Locale>,
    #[serde(skip_serializing_if = "Option::is_none")]
    features: Option<Vec<GuildFeature>>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    description: Patch<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    premium_progress_bar_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    safety_alerts_channel_id: Patch<ChannelId>,
}

impl EditGuild {
    /// Sets the name, 2 to 100 characters.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.body.name = Some(name.into());
        self
    }

    /// Sets the voice region. Deprecated by Discord: use the `rtc_region` of channels.
    pub fn region(mut self, region: impl Into<String>) -> Self {
        self.body.region = Patch::Set(region.into());
        self
    }

    /// Sets the verification level.
    pub fn verification_level(mut self, level: VerificationLevel) -> Self {
        self.body.verification_level = Some(level);
        self
    }

    /// Sets the default message notification level.
    pub fn default_message_notifications(mut self, level: DefaultMessageNotificationLevel) -> Self {
        self.body.default_message_notifications = Some(level);
        self
    }

    /// Sets the explicit content filter level.
    pub fn explicit_content_filter(mut self, level: ExplicitContentFilter) -> Self {
        self.body.explicit_content_filter = Some(level);
        self
    }

    /// Sets the AFK channel.
    pub fn afk_channel_id(mut self, channel_id: ChannelId) -> Self {
        self.body.afk_channel_id = Patch::Set(channel_id);
        self
    }

    /// Removes the AFK channel.
    pub fn clear_afk_channel_id(mut self) -> Self {
        self.body.afk_channel_id = Patch::Clear;
        self
    }

    /// Sets the AFK timeout in seconds: 60, 300, 900, 1800, or 3600.
    pub fn afk_timeout(mut self, seconds: u32) -> Self {
        self.body.afk_timeout = Some(seconds);
        self
    }

    /// Sets the icon, 1024x1024. An animated GIF needs the `ANIMATED_ICON` feature.
    pub fn icon(mut self, icon: ImageData) -> Self {
        self.body.icon = Patch::Set(icon);
        self
    }

    /// Removes the icon.
    pub fn clear_icon(mut self) -> Self {
        self.body.icon = Patch::Clear;
        self
    }

    /// Sets the invite splash, 16:9. Needs the `INVITE_SPLASH` feature.
    pub fn splash(mut self, splash: ImageData) -> Self {
        self.body.splash = Patch::Set(splash);
        self
    }

    /// Removes the invite splash.
    pub fn clear_splash(mut self) -> Self {
        self.body.splash = Patch::Clear;
        self
    }

    /// Sets the discovery splash, 16:9. Needs the `DISCOVERABLE` feature.
    pub fn discovery_splash(mut self, splash: ImageData) -> Self {
        self.body.discovery_splash = Patch::Set(splash);
        self
    }

    /// Removes the discovery splash.
    pub fn clear_discovery_splash(mut self) -> Self {
        self.body.discovery_splash = Patch::Clear;
        self
    }

    /// Sets the banner, 16:9. Needs the `BANNER` feature.
    pub fn banner(mut self, banner: ImageData) -> Self {
        self.body.banner = Patch::Set(banner);
        self
    }

    /// Removes the banner.
    pub fn clear_banner(mut self) -> Self {
        self.body.banner = Patch::Clear;
        self
    }

    /// Sets the channel for welcome messages and boost events.
    pub fn system_channel_id(mut self, channel_id: ChannelId) -> Self {
        self.body.system_channel_id = Patch::Set(channel_id);
        self
    }

    /// Removes the system channel.
    pub fn clear_system_channel_id(mut self) -> Self {
        self.body.system_channel_id = Patch::Clear;
        self
    }

    /// Sets the system channel flags.
    pub fn system_channel_flags(mut self, flags: SystemChannelFlags) -> Self {
        self.body.system_channel_flags = Some(flags);
        self
    }

    /// Sets the rules channel of a Community guild.
    pub fn rules_channel_id(mut self, channel_id: ChannelId) -> Self {
        self.body.rules_channel_id = Patch::Set(channel_id);
        self
    }

    /// Removes the rules channel.
    pub fn clear_rules_channel_id(mut self) -> Self {
        self.body.rules_channel_id = Patch::Clear;
        self
    }

    /// Sets the channel for notices from Discord to moderators of a Community guild.
    pub fn public_updates_channel_id(mut self, channel_id: ChannelId) -> Self {
        self.body.public_updates_channel_id = Patch::Set(channel_id);
        self
    }

    /// Removes the public updates channel.
    pub fn clear_public_updates_channel_id(mut self) -> Self {
        self.body.public_updates_channel_id = Patch::Clear;
        self
    }

    /// Sets the preferred locale of a Community guild.
    pub fn preferred_locale(mut self, locale: Locale) -> Self {
        self.body.preferred_locale = Patch::Set(locale);
        self
    }

    /// Sets the preferred locale back to the default (`en-US`).
    pub fn clear_preferred_locale(mut self) -> Self {
        self.body.preferred_locale = Patch::Clear;
        self
    }

    /// Sets the features. Discord ignores features that it does not let you change.
    ///
    /// To add or remove `COMMUNITY`, the current user needs `ADMINISTRATOR`.
    pub fn features(mut self, features: Vec<GuildFeature>) -> Self {
        self.body.features = Some(features);
        self
    }

    /// Sets the description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.body.description = Patch::Set(description.into());
        self
    }

    /// Removes the description.
    pub fn clear_description(mut self) -> Self {
        self.body.description = Patch::Clear;
        self
    }

    /// Shows or hides the boost progress bar.
    pub fn premium_progress_bar_enabled(mut self, enabled: bool) -> Self {
        self.body.premium_progress_bar_enabled = Some(enabled);
        self
    }

    /// Sets the channel for safety alerts to moderators of a Community guild.
    pub fn safety_alerts_channel_id(mut self, channel_id: ChannelId) -> Self {
        self.body.safety_alerts_channel_id = Patch::Set(channel_id);
        self
    }

    /// Removes the safety alerts channel.
    pub fn clear_safety_alerts_channel_id(mut self) -> Self {
        self.body.safety_alerts_channel_id = Patch::Clear;
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditGuild {
    type Response = Guild;

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}",
            format!("/guilds/{}", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            limits::optional_length("name", self.body.name.as_deref(), 2, 100)?;
            if let Some(timeout) = self.body.afk_timeout
                && !AFK_TIMEOUTS.contains(&timeout)
            {
                return Err(ValidationError::new(
                    "afk_timeout",
                    "must be 60, 300, 900, 1800, or 3600",
                ));
            }
            Ok(())
        })?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /guilds/{guild.id}/channels`: the channels of a guild, without threads.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildChannels {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildChannels {
    type Response = Vec<Channel>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/channels",
            format!("/guilds/{}/channels", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `POST /guilds/{guild.id}/channels`: makes a channel in a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateGuildChannel {
    client: Client,
    guild_id: GuildId,
    body: CreateGuildChannelBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct CreateGuildChannelBody {
    name: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<ChannelType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rate_limit_per_user: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    permission_overwrites: Option<Vec<PermissionOverwrite>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<ChannelId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nsfw: Option<bool>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    rtc_region: Patch<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    video_quality_mode: Option<VideoQualityMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_auto_archive_duration: Option<AutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_reaction_emoji: Option<DefaultReaction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    available_tags: Option<Vec<ForumTag>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_sort_order: Option<SortOrder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_forum_layout: Option<ForumLayout>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_thread_rate_limit_per_user: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<ChannelFlags>,
}

impl CreateGuildChannel {
    /// Sets the channel type. The default is a text channel.
    pub fn kind(mut self, kind: ChannelType) -> Self {
        self.body.kind = Some(kind);
        self
    }

    /// Sets the topic, 0 to 1024 characters.
    pub fn topic(mut self, topic: impl Into<String>) -> Self {
        self.body.topic = Some(topic.into());
        self
    }

    /// Sets the bitrate of a voice or stage channel in bits per second, at least 8000.
    pub fn bitrate(mut self, bitrate: u32) -> Self {
        self.body.bitrate = Some(bitrate);
        self
    }

    /// Sets the user limit of a voice or stage channel. 0 means no limit.
    pub fn user_limit(mut self, limit: u32) -> Self {
        self.body.user_limit = Some(limit);
        self
    }

    /// Sets the slow mode delay in seconds, 0 to 21600.
    pub fn rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.body.rate_limit_per_user = Some(seconds);
        self
    }

    /// Sets the position. Channels with the same position sort by ID.
    pub fn position(mut self, position: i32) -> Self {
        self.body.position = Some(position);
        self
    }

    /// Sets the permission overwrites.
    pub fn permission_overwrites(mut self, overwrites: Vec<PermissionOverwrite>) -> Self {
        self.body.permission_overwrites = Some(overwrites);
        self
    }

    /// Sets the parent category.
    pub fn parent_id(mut self, channel_id: ChannelId) -> Self {
        self.body.parent_id = Some(channel_id);
        self
    }

    /// Marks the channel as age-restricted.
    pub fn nsfw(mut self, nsfw: bool) -> Self {
        self.body.nsfw = Some(nsfw);
        self
    }

    /// Sets the voice region of a voice or stage channel.
    pub fn rtc_region(mut self, region: impl Into<String>) -> Self {
        self.body.rtc_region = Patch::Set(region.into());
        self
    }

    /// Sets the voice region to automatic (sends null).
    pub fn automatic_rtc_region(mut self) -> Self {
        self.body.rtc_region = Patch::Clear;
        self
    }

    /// Sets the camera video quality of a voice or stage channel.
    pub fn video_quality_mode(mut self, mode: VideoQualityMode) -> Self {
        self.body.video_quality_mode = Some(mode);
        self
    }

    /// Sets the default auto archive duration that clients use for new threads.
    pub fn default_auto_archive_duration(mut self, duration: AutoArchiveDuration) -> Self {
        self.body.default_auto_archive_duration = Some(duration);
        self
    }

    /// Sets the default reaction emoji of a forum or media channel.
    pub fn default_reaction_emoji(mut self, emoji: DefaultReaction) -> Self {
        self.body.default_reaction_emoji = Some(emoji);
        self
    }

    /// Sets the tags of a forum or media channel.
    pub fn available_tags(mut self, tags: Vec<ForumTag>) -> Self {
        self.body.available_tags = Some(tags);
        self
    }

    /// Sets the default sort order of a forum or media channel.
    pub fn default_sort_order(mut self, order: SortOrder) -> Self {
        self.body.default_sort_order = Some(order);
        self
    }

    /// Sets the default layout of a forum channel.
    pub fn default_forum_layout(mut self, layout: ForumLayout) -> Self {
        self.body.default_forum_layout = Some(layout);
        self
    }

    /// Sets the slow mode delay of new threads in seconds, 0 to 21600.
    pub fn default_thread_rate_limit_per_user(mut self, seconds: u32) -> Self {
        self.body.default_thread_rate_limit_per_user = Some(seconds);
        self
    }

    /// Sets the channel flags.
    pub fn flags(mut self, flags: ChannelFlags) -> Self {
        self.body.flags = Some(flags);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for CreateGuildChannel {
    type Response = Channel;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/guilds/{guild_id}/channels",
            format!("/guilds/{}/channels", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            let body = &self.body;
            limits::length("name", &body.name, 1, 100)?;
            limits::optional_length("topic", body.topic.as_deref(), 0, 1024)?;
            limits::optional_range(
                "rate_limit_per_user",
                body.rate_limit_per_user.map(u64::from),
                0,
                RATE_LIMIT_PER_USER_MAX,
            )?;
            limits::optional_range(
                "default_thread_rate_limit_per_user",
                body.default_thread_rate_limit_per_user.map(u64::from),
                0,
                RATE_LIMIT_PER_USER_MAX,
            )?;
            limits::optional_range("bitrate", body.bitrate.map(u64::from), 8000, u64::MAX)
        })?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// A change of the position or the parent of one channel, for
/// [`Client::edit_guild_channel_positions`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct ChannelPosition {
    id: ChannelId,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lock_permissions: Option<bool>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    parent_id: Patch<ChannelId>,
}

impl ChannelPosition {
    /// Makes a change for a channel. It changes nothing until a setter is used.
    #[must_use]
    pub const fn new(channel_id: ChannelId) -> Self {
        Self {
            id: channel_id,
            position: None,
            lock_permissions: None,
            parent_id: Patch::Keep,
        }
    }

    /// Sets the position. Channels with the same position sort by ID.
    #[must_use]
    pub const fn position(mut self, position: i32) -> Self {
        self.position = Some(position);
        self
    }

    /// Moves the channel to a category.
    #[must_use]
    pub const fn parent_id(mut self, parent_id: ChannelId) -> Self {
        self.parent_id = Patch::Set(parent_id);
        self
    }

    /// Moves the channel out of its category.
    #[must_use]
    pub const fn clear_parent_id(mut self) -> Self {
        self.parent_id = Patch::Clear;
        self
    }

    /// Copies the permission overwrites of the new parent category. Needs `MANAGE_ROLES`.
    #[must_use]
    pub const fn lock_permissions(mut self, lock: bool) -> Self {
        self.lock_permissions = Some(lock);
        self
    }
}

/// `PATCH /guilds/{guild.id}/channels`: changes the positions of channels.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildChannelPositions {
    client: Client,
    guild_id: GuildId,
    positions: Vec<ChannelPosition>,
    reason: Option<String>,
}

impl EditGuildChannelPositions {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for EditGuildChannelPositions {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Patch,
            "/guilds/{guild_id}/channels",
            format!("/guilds/{}/channels", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            let parent_changes = self
                .positions
                .iter()
                .filter(|position| !position.parent_id.is_keep())
                .count();
            if parent_changes > 1 {
                return Err(ValidationError::new(
                    "parent_id",
                    "only one channel in a request can change its parent",
                ));
            }
            Ok(())
        })?;
        RequestBody::json(&self.positions)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /guilds/{guild.id}/threads/active`: the active threads of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetActiveGuildThreads {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetActiveGuildThreads {
    type Response = ThreadList;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/threads/active",
            format!("/guilds/{}/threads/active", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/prune`: the number of members that a prune removes.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildPruneCount {
    client: Client,
    guild_id: GuildId,
    days: Option<u8>,
    include_roles: Vec<RoleId>,
}

impl GetGuildPruneCount {
    /// Sets the number of days without activity, 1 to 30. The default is 7.
    pub fn days(mut self, days: u8) -> Self {
        self.days = Some(days.clamp(1, 30));
        self
    }

    /// Counts members that have only these roles. By default, members with roles stay.
    pub fn include_roles(mut self, role_ids: Vec<RoleId>) -> Self {
        self.include_roles = role_ids;
        self
    }
}

impl Endpoint for GetGuildPruneCount {
    type Response = PruneCount;

    fn route(&self) -> Route {
        let mut query = Query::new();
        query.push_opt("days", self.days);
        if !self.include_roles.is_empty() {
            let role_ids: Vec<String> =
                self.include_roles.iter().map(ToString::to_string).collect();
            query.push("include_roles", role_ids.join(","));
        }
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/prune",
            format!("/guilds/{}/prune{}", self.guild_id, query.finish()),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `POST /guilds/{guild.id}/prune`: removes members without activity.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct BeginGuildPrune {
    client: Client,
    guild_id: GuildId,
    body: BeginGuildPruneBody,
    reason: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
struct BeginGuildPruneBody {
    #[serde(skip_serializing_if = "Option::is_none")]
    days: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compute_prune_count: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    include_roles: Option<Vec<RoleId>>,
}

impl BeginGuildPrune {
    /// Sets the number of days without activity, 1 to 30. The default is 7.
    pub fn days(mut self, days: u8) -> Self {
        self.body.days = Some(days);
        self
    }

    /// Tells if the response has the number of removed members. The default is `true`.
    ///
    /// Use `false` for large guilds.
    pub fn compute_prune_count(mut self, compute: bool) -> Self {
        self.body.compute_prune_count = Some(compute);
        self
    }

    /// Removes members that have only these roles. By default, members with roles stay.
    pub fn include_roles(mut self, role_ids: Vec<RoleId>) -> Self {
        self.body.include_roles = Some(role_ids);
        self
    }

    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for BeginGuildPrune {
    type Response = PruneCount;

    fn route(&self) -> Route {
        Route::new(
            Method::Post,
            "/guilds/{guild_id}/prune",
            format!("/guilds/{}/prune", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, validate: bool) -> Result<RequestBody, Error> {
        check(validate, || {
            limits::optional_range("days", self.body.days.map(u64::from), 1, 30)
        })?;
        RequestBody::json(&self.body)
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /guilds/{guild.id}/regions`: the voice regions of a guild, with VIP regions.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildVoiceRegions {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildVoiceRegions {
    type Response = Vec<VoiceRegion>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/regions",
            format!("/guilds/{}/regions", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/invites`: the invites of a guild.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildInvites {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildInvites {
    type Response = Vec<Invite>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/invites",
            format!("/guilds/{}/invites", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `GET /guilds/{guild.id}/integrations`: the integrations of a guild, up to 50.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildIntegrations {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildIntegrations {
    type Response = Vec<Integration>;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/integrations",
            format!("/guilds/{}/integrations", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `DELETE /guilds/{guild.id}/integrations/{integration.id}`: removes an integration.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteGuildIntegration {
    client: Client,
    guild_id: GuildId,
    integration_id: IntegrationId,
    reason: Option<String>,
}

impl DeleteGuildIntegration {
    /// Sets the reason for the audit log.
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

impl Endpoint for DeleteGuildIntegration {
    type Response = ();

    fn route(&self) -> Route {
        Route::new(
            Method::Delete,
            "/guilds/{guild_id}/integrations/{integration_id}",
            format!(
                "/guilds/{}/integrations/{}",
                self.guild_id, self.integration_id
            ),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn audit_reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

/// `GET /guilds/{guild.id}/vanity-url`: the vanity invite of a guild and its uses.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct GetGuildVanityUrl {
    client: Client,
    guild_id: GuildId,
}

impl Endpoint for GetGuildVanityUrl {
    type Response = VanityUrl;

    fn route(&self) -> Route {
        Route::new(
            Method::Get,
            "/guilds/{guild_id}/vanity-url",
            format!("/guilds/{}/vanity-url", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }
}

/// `PUT /guilds/{guild.id}/incident-actions`: pauses or resumes invites and direct messages.
#[derive(Clone, Debug)]
#[must_use = "a request does nothing until it is awaited"]
pub struct EditGuildIncidentActions {
    client: Client,
    guild_id: GuildId,
    body: EditGuildIncidentActionsBody,
}

#[derive(Clone, Debug, Default, Serialize)]
struct EditGuildIncidentActionsBody {
    #[serde(skip_serializing_if = "Patch::is_keep")]
    invites_disabled_until: Patch<Timestamp>,
    #[serde(skip_serializing_if = "Patch::is_keep")]
    dms_disabled_until: Patch<Timestamp>,
}

impl EditGuildIncidentActions {
    /// Pauses invites until a time, at most 24 hours in the future.
    pub fn invites_disabled_until(mut self, until: Timestamp) -> Self {
        self.body.invites_disabled_until = Patch::Set(until);
        self
    }

    /// Resumes invites.
    pub fn enable_invites(mut self) -> Self {
        self.body.invites_disabled_until = Patch::Clear;
        self
    }

    /// Pauses direct messages until a time, at most 24 hours in the future.
    pub fn dms_disabled_until(mut self, until: Timestamp) -> Self {
        self.body.dms_disabled_until = Patch::Set(until);
        self
    }

    /// Resumes direct messages.
    pub fn enable_dms(mut self) -> Self {
        self.body.dms_disabled_until = Patch::Clear;
        self
    }
}

impl Endpoint for EditGuildIncidentActions {
    type Response = IncidentsData;

    fn route(&self) -> Route {
        Route::new(
            Method::Put,
            "/guilds/{guild_id}/incident-actions",
            format!("/guilds/{}/incident-actions", self.guild_id),
            MajorParameter::Guild(self.guild_id),
        )
    }

    fn body(&self, _validate: bool) -> Result<RequestBody, Error> {
        RequestBody::json(&self.body)
    }
}

super::into_future!(
    GetGuild,
    GetGuildPreview,
    EditGuild,
    GetGuildChannels,
    CreateGuildChannel,
    EditGuildChannelPositions,
    GetActiveGuildThreads,
    GetGuildPruneCount,
    BeginGuildPrune,
    GetGuildVoiceRegions,
    GetGuildInvites,
    GetGuildIntegrations,
    DeleteGuildIntegration,
    GetGuildVanityUrl,
    EditGuildIncidentActions,
);

impl Client {
    /// Gets a guild.
    pub fn guild(&self, guild_id: GuildId) -> GetGuild {
        GetGuild {
            client: self.clone(),
            guild_id,
            with_counts: None,
        }
    }

    /// Gets the preview of a guild. Outside the guild, the guild must be discoverable.
    pub fn guild_preview(&self, guild_id: GuildId) -> GetGuildPreview {
        GetGuildPreview {
            client: self.clone(),
            guild_id,
        }
    }

    /// Changes the settings of a guild. Needs `MANAGE_GUILD`.
    pub fn edit_guild(&self, guild_id: GuildId) -> EditGuild {
        EditGuild {
            client: self.clone(),
            guild_id,
            body: EditGuildBody::default(),
            reason: None,
        }
    }

    /// Gets the channels of a guild, without threads.
    pub fn guild_channels(&self, guild_id: GuildId) -> GetGuildChannels {
        GetGuildChannels {
            client: self.clone(),
            guild_id,
        }
    }

    /// Makes a channel in a guild. Needs `MANAGE_CHANNELS`.
    pub fn create_guild_channel(
        &self,
        guild_id: GuildId,
        name: impl Into<String>,
    ) -> CreateGuildChannel {
        CreateGuildChannel {
            client: self.clone(),
            guild_id,
            body: CreateGuildChannelBody {
                name: name.into(),
                ..CreateGuildChannelBody::default()
            },
            reason: None,
        }
    }

    /// Changes the positions or the parents of channels. Needs `MANAGE_CHANNELS`.
    ///
    /// Only one channel in a request can change its parent.
    pub fn edit_guild_channel_positions(
        &self,
        guild_id: GuildId,
        positions: Vec<ChannelPosition>,
    ) -> EditGuildChannelPositions {
        EditGuildChannelPositions {
            client: self.clone(),
            guild_id,
            positions,
            reason: None,
        }
    }

    /// Gets the active threads of a guild, public and private.
    pub fn active_guild_threads(&self, guild_id: GuildId) -> GetActiveGuildThreads {
        GetActiveGuildThreads {
            client: self.clone(),
            guild_id,
        }
    }

    /// Gets the number of members that a prune removes.
    pub fn guild_prune_count(&self, guild_id: GuildId) -> GetGuildPruneCount {
        GetGuildPruneCount {
            client: self.clone(),
            guild_id,
            days: None,
            include_roles: Vec::new(),
        }
    }

    /// Removes members without activity. Needs `MANAGE_GUILD` and `KICK_MEMBERS`.
    pub fn begin_guild_prune(&self, guild_id: GuildId) -> BeginGuildPrune {
        BeginGuildPrune {
            client: self.clone(),
            guild_id,
            body: BeginGuildPruneBody::default(),
            reason: None,
        }
    }

    /// Gets the voice regions of a guild, with VIP regions when the guild has them.
    pub fn guild_voice_regions(&self, guild_id: GuildId) -> GetGuildVoiceRegions {
        GetGuildVoiceRegions {
            client: self.clone(),
            guild_id,
        }
    }

    /// Gets the invites of a guild. Needs `MANAGE_GUILD` or `VIEW_AUDIT_LOG`.
    pub fn guild_invites(&self, guild_id: GuildId) -> GetGuildInvites {
        GetGuildInvites {
            client: self.clone(),
            guild_id,
        }
    }

    /// Gets the integrations of a guild, up to 50. Needs `MANAGE_GUILD`.
    pub fn guild_integrations(&self, guild_id: GuildId) -> GetGuildIntegrations {
        GetGuildIntegrations {
            client: self.clone(),
            guild_id,
        }
    }

    /// Removes an integration, its webhooks, and its bot. Needs `MANAGE_GUILD`.
    pub fn delete_guild_integration(
        &self,
        guild_id: GuildId,
        integration_id: IntegrationId,
    ) -> DeleteGuildIntegration {
        DeleteGuildIntegration {
            client: self.clone(),
            guild_id,
            integration_id,
            reason: None,
        }
    }

    /// Gets the vanity invite of a guild and its uses. Needs `MANAGE_GUILD`.
    pub fn guild_vanity_url(&self, guild_id: GuildId) -> GetGuildVanityUrl {
        GetGuildVanityUrl {
            client: self.clone(),
            guild_id,
        }
    }

    /// Pauses or resumes invites and direct messages in a guild. Needs `MANAGE_GUILD`.
    pub fn edit_guild_incident_actions(&self, guild_id: GuildId) -> EditGuildIncidentActions {
        EditGuildIncidentActions {
            client: self.clone(),
            guild_id,
            body: EditGuildIncidentActionsBody::default(),
        }
    }
}
