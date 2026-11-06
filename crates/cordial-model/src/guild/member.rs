//! Guild members.

use serde::{Deserialize, Serialize};

use crate::id::{GuildId, RoleId, UserId};
use crate::user::{AvatarDecorationData, Collectibles, User};
use crate::{ImageHash, Permissions, Timestamp};

/// A member of a guild.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Member {
    /// The guild ID. Only in gateway events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The user. Absent in the members of `MESSAGE_CREATE` and `MESSAGE_UPDATE` events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<User>,
    /// The nickname in the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nick: Option<String>,
    /// The avatar hash in the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<ImageHash>,
    /// The banner hash in the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner: Option<ImageHash>,
    /// The role IDs.
    #[serde(default)]
    pub roles: Vec<RoleId>,
    /// When the user joined. `None` for a guest that a voice invite added.
    #[serde(default)]
    pub joined_at: Option<Timestamp>,
    /// When the user started to boost the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premium_since: Option<Timestamp>,
    /// Tells if the user is deafened in voice channels.
    #[serde(default)]
    pub deaf: bool,
    /// Tells if the user is muted in voice channels.
    #[serde(default)]
    pub mute: bool,
    /// The member flags.
    #[serde(default)]
    pub flags: MemberFlags,
    /// Tells if the user has not passed Membership Screening yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<bool>,
    /// The permissions of the member in the channel, with overwrites. Only in interactions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,
    /// When the timeout of the user ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub communication_disabled_until: Option<Timestamp>,
    /// The avatar decoration in the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
    /// The collectibles in the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collectibles: Option<Collectibles>,
}

impl Member {
    /// Gives the user ID, if the member has a user.
    #[must_use]
    pub fn user_id(&self) -> Option<UserId> {
        self.user.as_ref().map(|user| user.id)
    }

    /// Gives the name to show: the nickname, then the display name, then the username.
    #[must_use]
    pub fn display_name(&self) -> Option<&str> {
        self.nick
            .as_deref()
            .or_else(|| self.user.as_ref().map(User::display_name))
    }

    /// Tells if the member is timed out at the time `now`.
    #[must_use]
    pub fn is_timed_out_at(&self, now: Timestamp) -> bool {
        self.communication_disabled_until
            .is_some_and(|until| until > now)
    }
}

crate::discord_flags! {
    /// Flags of a guild member.
    pub struct MemberFlags: u64 as integer {
        /// The member left and joined the guild again.
        const DID_REJOIN = 1 << 0;
        /// The member completed onboarding.
        const COMPLETED_ONBOARDING = 1 << 1;
        /// The member is exempt from the verification requirements. Bots can change it.
        const BYPASSES_VERIFICATION = 1 << 2;
        /// The member started onboarding.
        const STARTED_ONBOARDING = 1 << 3;
        /// The member is a guest. A guest can only use the voice channel of the invite.
        const IS_GUEST = 1 << 4;
        /// The member started the new member actions of the Server Guide.
        const STARTED_HOME_ACTIONS = 1 << 5;
        /// The member completed the new member actions of the Server Guide.
        const COMPLETED_HOME_ACTIONS = 1 << 6;
        /// AutoMod blocks the username, display name, or nickname of the member.
        const AUTOMOD_QUARANTINED_USERNAME = 1 << 7;
        /// The member dismissed the DM settings upsell.
        const DM_SETTINGS_UPSELL_ACKNOWLEDGED = 1 << 9;
        /// AutoMod blocks the guild tag of the member.
        const AUTOMOD_QUARANTINED_GUILD_TAG = 1 << 10;
    }
}

/// A partial member, as in interactions and in the mentions of messages.
///
/// It has no `user`, `deaf`, or `mute` field.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PartialMember {
    /// The nickname in the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nick: Option<String>,
    /// The avatar hash in the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar: Option<ImageHash>,
    /// The banner hash in the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner: Option<ImageHash>,
    /// The role IDs.
    #[serde(default)]
    pub roles: Vec<RoleId>,
    /// When the user joined.
    #[serde(default)]
    pub joined_at: Option<Timestamp>,
    /// When the user started to boost the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premium_since: Option<Timestamp>,
    /// The member flags.
    #[serde(default)]
    pub flags: MemberFlags,
    /// Tells if the user has not passed Membership Screening yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<bool>,
    /// The permissions of the member in the channel, with overwrites.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permissions: Option<Permissions>,
    /// When the timeout of the user ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub communication_disabled_until: Option<Timestamp>,
    /// The avatar decoration in the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_decoration_data: Option<AvatarDecorationData>,
    /// The collectibles in the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collectibles: Option<Collectibles>,
    /// Tells if the user is deafened in voice channels. Usually absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deaf: Option<bool>,
    /// Tells if the user is muted in voice channels. Usually absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mute: Option<bool>,
}

impl PartialMember {
    /// Makes a full member with the user.
    #[must_use]
    pub fn into_member(self, user: User) -> Member {
        Member {
            guild_id: None,
            user: Some(user),
            nick: self.nick,
            avatar: self.avatar,
            banner: self.banner,
            roles: self.roles,
            joined_at: self.joined_at,
            premium_since: self.premium_since,
            deaf: self.deaf.unwrap_or(false),
            mute: self.mute.unwrap_or(false),
            flags: self.flags,
            pending: self.pending,
            permissions: self.permissions,
            communication_disabled_until: self.communication_disabled_until,
            avatar_decoration_data: self.avatar_decoration_data,
            collectibles: self.collectibles,
        }
    }
}
