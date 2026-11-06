//! Channels, threads, forum tags, and related objects.

use serde::{Deserialize, Serialize};

use crate::guild::Member;
use crate::id::{
    ApplicationId, ChannelId, EmojiId, ForumTagId, GuildId, MessageId, UserId, WebhookId,
};
use crate::permissions::{ChannelKind, PermissionOverwrite};
use crate::user::User;
use crate::{ImageHash, Permissions, Timestamp};

/// A channel in a guild, a DM channel, or a thread.
///
/// Discord uses one object for all channel kinds. Many fields apply only to some kinds.
/// Over the gateway, a channel that the bot cannot view arrives obfuscated:
/// see [`Channel::is_obfuscated`].
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Channel {
    /// The channel ID.
    pub id: ChannelId,
    /// The channel type.
    #[serde(rename = "type")]
    pub kind: ChannelType,
    /// The guild ID. It can be absent in channels inside gateway guild events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The sort position. Channels with the same position sort by ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<i32>,
    /// The permission overwrites.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_overwrites: Option<Vec<PermissionOverwrite>>,
    /// The name, 1 to 100 characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The topic: up to 4096 characters for forum and media channels, else up to 1024.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    /// Tells if the channel is age-restricted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nsfw: Option<bool>,
    /// The ID of the last message (or thread, in forum and media channels). It can refer to a
    /// message that does not exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_message_id: Option<MessageId>,
    /// The bitrate of a voice channel, in bits per second.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    /// The user limit of a voice channel. 0 means no limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_limit: Option<u32>,
    /// The slowmode in seconds (0 to 21600).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_per_user: Option<u32>,
    /// The recipients of a DM channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipients: Option<Vec<User>>,
    /// The icon hash of a group DM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<ImageHash>,
    /// The user that made the group DM or the thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<UserId>,
    /// The application of the channel. For a group DM, the application that made it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
    /// For a group DM: tells if an application manages it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed: Option<bool>,
    /// For a guild channel: the parent category. For a thread: the channel of the thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<ChannelId>,
    /// When the last message was pinned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_pin_timestamp: Option<Timestamp>,
    /// The voice region of a voice channel. `None` means automatic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rtc_region: Option<String>,
    /// The camera video quality of a voice channel. Absent means `Auto`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_quality_mode: Option<VideoQualityMode>,
    /// The number of messages in a thread, without the first message and deleted messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_count: Option<u32>,
    /// The approximate number of users in a thread. It stops at 50.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u32>,
    /// The thread metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_metadata: Option<ThreadMetadata>,
    /// The thread member of the current user, on some endpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<ThreadMember>,
    /// The default auto-archive duration of new threads, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_auto_archive_duration: Option<AutoArchiveDuration>,
    /// The permissions of the invoking user, with overwrites. Only in interaction data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,
    /// The permissions of the bot, with overwrites. Only in interaction data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_permissions: Option<Permissions>,
    /// The channel flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<ChannelFlags>,
    /// The number of messages ever sent in a thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_message_sent: Option<u32>,
    /// The tags that threads in a forum or media channel can have.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_tags: Option<Vec<ForumTag>>,
    /// The tags of a thread in a forum or media channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_tags: Option<Vec<ForumTagId>>,
    /// The emoji of the reaction button on posts in a forum or media channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_reaction_emoji: Option<DefaultReaction>,
    /// The initial slowmode of new threads.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_thread_rate_limit_per_user: Option<u32>,
    /// The default sort order of posts in a forum or media channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_sort_order: Option<SortOrder>,
    /// The default layout of a forum channel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_forum_layout: Option<ForumLayout>,
    /// In `THREAD_CREATE`: tells if the thread is new, not an existing thread that the
    /// current user was added to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub newly_created: Option<bool>,
}

impl Channel {
    /// Gives the channel flags, or no flags when the field is absent.
    #[must_use]
    pub fn flags(&self) -> ChannelFlags {
        self.flags.unwrap_or_default()
    }

    /// Tells if Discord obfuscated the channel because the bot cannot view it.
    ///
    /// Only `id`, `kind`, `position`, and `parent_id` are reliable on an obfuscated channel.
    #[must_use]
    pub fn is_obfuscated(&self) -> bool {
        self.flags().contains(ChannelFlags::CHANNEL_OBFUSCATED)
    }

    /// Tells if the channel is a thread.
    #[must_use]
    pub fn is_thread(&self) -> bool {
        self.kind.is_thread()
    }

    /// Gives the mention text, for example `<#41771983423143936>`.
    #[must_use]
    pub fn mention(&self) -> String {
        format!("<#{}>", self.id)
    }
}

crate::discord_enum! {
    /// The type of a channel.
    pub enum ChannelType: u8 {
        /// A text channel in a guild.
        GuildText = 0,
        /// A direct message between two users.
        Dm = 1,
        /// A voice channel in a guild.
        GuildVoice = 2,
        /// A direct message between many users.
        GroupDm = 3,
        /// A category that contains up to 50 channels.
        GuildCategory = 4,
        /// A channel that users can follow and crosspost into their own guild.
        GuildAnnouncement = 5,
        /// A thread in an announcement channel.
        AnnouncementThread = 10,
        /// A public thread in a text or forum channel.
        PublicThread = 11,
        /// A private thread in a text channel.
        PrivateThread = 12,
        /// A voice channel for events with an audience.
        GuildStageVoice = 13,
        /// The channel in a hub that lists the guilds of the hub.
        GuildDirectory = 14,
        /// A channel that contains only threads.
        GuildForum = 15,
        /// A channel that contains only threads, with a media layout. Still in development.
        GuildMedia = 16,
    }
}

impl ChannelType {
    /// Tells if the type is a thread.
    #[must_use]
    pub const fn is_thread(self) -> bool {
        matches!(
            self,
            Self::AnnouncementThread | Self::PublicThread | Self::PrivateThread
        )
    }

    /// Tells if the type is a voice or stage channel.
    #[must_use]
    pub const fn is_voice(self) -> bool {
        matches!(self, Self::GuildVoice | Self::GuildStageVoice)
    }

    /// Tells if the type is a forum or media channel.
    #[must_use]
    pub const fn is_forum_like(self) -> bool {
        matches!(self, Self::GuildForum | Self::GuildMedia)
    }

    /// Tells if the type is a guild channel (not a DM).
    #[must_use]
    pub const fn is_guild(self) -> bool {
        !matches!(self, Self::Dm | Self::GroupDm)
    }

    /// Gives the kind for the implicit permission rules.
    #[must_use]
    pub const fn permission_kind(self) -> ChannelKind {
        match self {
            Self::GuildText | Self::GuildAnnouncement | Self::GuildForum | Self::GuildMedia => {
                ChannelKind::Text
            }
            Self::GuildVoice | Self::GuildStageVoice => ChannelKind::Voice,
            Self::AnnouncementThread | Self::PublicThread | Self::PrivateThread => {
                ChannelKind::Thread
            }
            _ => ChannelKind::Other,
        }
    }
}

crate::discord_enum! {
    /// The camera video quality of a voice channel.
    pub enum VideoQualityMode: u8 {
        /// Discord chooses the quality.
        Auto = 1,
        /// 720p.
        Full = 2,
    }
}

crate::discord_flags! {
    /// Flags of a channel.
    pub struct ChannelFlags: u64 as integer {
        /// The thread is pinned to the top of its forum or media channel.
        const PINNED = 1 << 1;
        /// New threads in the forum or media channel must have a tag.
        const REQUIRE_TAG = 1 << 4;
        /// The media channel hides the download options of embedded media.
        const HIDE_MEDIA_DOWNLOAD_OPTIONS = 1 << 15;
        /// Discord obfuscated the channel because the bot cannot view it. Gateway only.
        const CHANNEL_OBFUSCATED = 1 << 17;
        /// The channel is a spoiler channel. Users must opt in to view it.
        const IS_SPOILER_CHANNEL = 1 << 21;
    }
}

crate::discord_enum! {
    /// The default sort order of posts in a forum or media channel.
    pub enum SortOrder: u8 {
        /// Sort by activity.
        LatestActivity = 0,
        /// Sort by creation time, newest first.
        CreationDate = 1,
    }
}

crate::discord_enum! {
    /// The default layout of a forum channel.
    pub enum ForumLayout: u8 {
        /// No default layout.
        NotSet = 0,
        /// A list.
        ListView = 1,
        /// A gallery of tiles.
        GalleryView = 2,
    }
}

crate::discord_enum! {
    /// The time of inactivity, in minutes, after which a thread is archived.
    pub enum AutoArchiveDuration: u16 {
        /// One hour.
        Hour = 60,
        /// One day.
        Day = 1440,
        /// Three days.
        ThreeDays = 4320,
        /// One week.
        Week = 10080,
    }
}

/// A channel that a webhook follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct FollowedChannel {
    /// The source channel.
    pub channel_id: ChannelId,
    /// The webhook in the target channel.
    pub webhook_id: WebhookId,
}

/// The fields of a channel that apply only to threads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ThreadMetadata {
    /// Tells if the thread is archived.
    #[serde(default)]
    pub archived: bool,
    /// The time of inactivity, in minutes, after which the thread is archived.
    #[serde(default = "default_auto_archive_duration")]
    pub auto_archive_duration: AutoArchiveDuration,
    /// When the archive status last changed.
    pub archive_timestamp: Timestamp,
    /// Tells if the thread is locked. Only users with `MANAGE_THREADS` can unarchive it.
    #[serde(default)]
    pub locked: bool,
    /// For a private thread: tells if members that are not moderators can add others.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invitable: Option<bool>,
    /// When the thread was made. Only for threads made after 2022-01-09.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub create_timestamp: Option<Timestamp>,
}

fn default_auto_archive_duration() -> AutoArchiveDuration {
    AutoArchiveDuration::Day
}

/// A user that joined a thread.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ThreadMember {
    /// The guild ID. Only in gateway events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The thread ID. Absent in the threads of `GUILD_CREATE`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<ChannelId>,
    /// The user ID. Absent in the threads of `GUILD_CREATE`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<UserId>,
    /// When the user last joined the thread.
    pub join_timestamp: Timestamp,
    /// The settings of the user for the thread (notifications).
    #[serde(default)]
    pub flags: u64,
    /// The member. Only with `with_member=true` on the thread member endpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
}

/// The default reaction emoji of a forum or media channel. Exactly one field is set.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct DefaultReaction {
    /// The ID of a custom emoji.
    #[serde(default)]
    pub emoji_id: Option<EmojiId>,
    /// A Unicode emoji.
    #[serde(default)]
    pub emoji_name: Option<String>,
}

/// A tag that threads in a forum or media channel can have.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ForumTag {
    /// The tag ID. Absent when a request makes a new tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<ForumTagId>,
    /// The name, up to 20 characters.
    pub name: String,
    /// Tells if only members with `MANAGE_THREADS` can add or remove the tag.
    #[serde(default)]
    pub moderated: bool,
    /// The ID of a custom emoji. At most one of the two emoji fields is set.
    #[serde(default)]
    pub emoji_id: Option<EmojiId>,
    /// A Unicode emoji. At most one of the two emoji fields is set.
    #[serde(default)]
    pub emoji_name: Option<String>,
}

impl ForumTag {
    /// Makes a new tag for a request.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
            moderated: false,
            emoji_id: None,
            emoji_name: None,
        }
    }
}

/// A list of threads with the thread members of the current user.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ThreadList {
    /// The threads.
    pub threads: Vec<Channel>,
    /// The thread members of the current user, for the threads that the user joined.
    pub members: Vec<ThreadMember>,
    /// Tells if more threads are available. Only in the archived thread endpoints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_more: Option<bool>,
}

/// The status and the start time of a voice channel, as in `CHANNEL_INFO`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct VoiceChannelInfo {
    /// The channel ID.
    pub id: ChannelId,
    /// The voice channel status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// When the current voice session started.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::time::option_unix_seconds"
    )]
    pub voice_start_time: Option<Timestamp>,
}
