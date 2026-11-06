//! The compact types that the cache stores.

use std::ops::Deref;

use cordial_model::ImageHash;
use cordial_model::Timestamp;
use cordial_model::gateway::event::GuildMemberUpdate;
use cordial_model::guild::{Guild, Member, MemberFlags, PartialMember, Role};
use cordial_model::id::{GuildId, RoleId, UserId};
use cordial_model::user::{AvatarDecorationData, Collectibles};

/// A guild without its lists of objects (roles, channels, members, and others).
///
/// The cache keeps those objects separately. Use the cache methods to read them.
#[derive(Clone, Debug, PartialEq)]
pub struct CachedGuild(pub(crate) Guild);

impl CachedGuild {
    /// Makes a cached guild. The lists are emptied to save memory.
    pub(crate) fn new(mut guild: Guild) -> Self {
        guild.roles = Vec::new();
        guild.emojis = Vec::new();
        guild.stickers = None;
        guild.members = None;
        guild.channels = None;
        guild.threads = None;
        guild.presences = None;
        guild.voice_states = None;
        guild.stage_instances = None;
        guild.guild_scheduled_events = None;
        guild.soundboard_sounds = None;
        Self(guild)
    }

    /// Gives the guild.
    #[must_use]
    pub fn guild(&self) -> &Guild {
        &self.0
    }
}

impl Deref for CachedGuild {
    type Target = Guild;

    fn deref(&self) -> &Guild {
        &self.0
    }
}

/// A role with the ID of its guild.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct CachedRole {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The role.
    pub role: Role,
}

impl Deref for CachedRole {
    type Target = Role;

    fn deref(&self) -> &Role {
        &self.role
    }
}

/// A member without its user. Read the user with `Cache::user`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct CachedMember {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The user ID.
    pub user_id: UserId,
    /// The nickname.
    pub nick: Option<String>,
    /// The avatar hash in the guild.
    pub avatar: Option<ImageHash>,
    /// The banner hash in the guild.
    pub banner: Option<ImageHash>,
    /// The role IDs.
    pub roles: Vec<RoleId>,
    /// When the user joined.
    pub joined_at: Option<Timestamp>,
    /// When the user started to boost the guild.
    pub premium_since: Option<Timestamp>,
    /// Tells if the user is deafened.
    pub deaf: bool,
    /// Tells if the user is muted.
    pub mute: bool,
    /// The member flags.
    pub flags: MemberFlags,
    /// Tells if the user has not passed Membership Screening.
    pub pending: bool,
    /// When the timeout ends.
    pub communication_disabled_until: Option<Timestamp>,
    /// The avatar decoration in the guild.
    pub avatar_decoration_data: Option<AvatarDecorationData>,
    /// The collectibles in the guild.
    pub collectibles: Option<Collectibles>,
}

impl CachedMember {
    pub(crate) fn from_member(guild_id: GuildId, user_id: UserId, member: &Member) -> Self {
        Self {
            guild_id,
            user_id,
            nick: member.nick.clone(),
            avatar: member.avatar.clone(),
            banner: member.banner.clone(),
            roles: member.roles.clone(),
            joined_at: member.joined_at,
            premium_since: member.premium_since,
            deaf: member.deaf,
            mute: member.mute,
            flags: member.flags,
            pending: member.pending.unwrap_or(false),
            communication_disabled_until: member.communication_disabled_until,
            avatar_decoration_data: member.avatar_decoration_data.clone(),
            collectibles: member.collectibles.clone(),
        }
    }

    pub(crate) fn from_partial(guild_id: GuildId, user_id: UserId, member: &PartialMember) -> Self {
        Self {
            guild_id,
            user_id,
            nick: member.nick.clone(),
            avatar: member.avatar.clone(),
            banner: member.banner.clone(),
            roles: member.roles.clone(),
            joined_at: member.joined_at,
            premium_since: member.premium_since,
            deaf: member.deaf.unwrap_or(false),
            mute: member.mute.unwrap_or(false),
            flags: member.flags,
            pending: member.pending.unwrap_or(false),
            communication_disabled_until: member.communication_disabled_until,
            avatar_decoration_data: member.avatar_decoration_data.clone(),
            collectibles: member.collectibles.clone(),
        }
    }

    /// Applies a `GUILD_MEMBER_UPDATE`. Fields that the update does not have stay.
    pub(crate) fn apply_update(&mut self, update: &GuildMemberUpdate) {
        self.roles.clone_from(&update.roles);
        self.nick.clone_from(&update.nick);
        self.avatar.clone_from(&update.avatar);
        self.banner.clone_from(&update.banner);
        self.joined_at = update.joined_at;
        self.premium_since = update.premium_since;
        self.communication_disabled_until = update.communication_disabled_until;
        if let Some(deaf) = update.deaf {
            self.deaf = deaf;
        }
        if let Some(mute) = update.mute {
            self.mute = mute;
        }
        if let Some(pending) = update.pending {
            self.pending = pending;
        }
        if let Some(flags) = update.flags {
            self.flags = flags;
        }
        if update.avatar_decoration_data.is_some() {
            self.avatar_decoration_data
                .clone_from(&update.avatar_decoration_data);
        }
        if update.collectibles.is_some() {
            self.collectibles.clone_from(&update.collectibles);
        }
    }

    /// Tells if the member is timed out at the time `now`.
    #[must_use]
    pub fn is_timed_out_at(&self, now: Timestamp) -> bool {
        self.communication_disabled_until
            .is_some_and(|until| until > now)
    }
}
