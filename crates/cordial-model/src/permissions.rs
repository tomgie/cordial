//! Permission flags, permission overwrites, and the permission calculator.
//!
//! The algorithm follows the "Permissions" topic of the Discord docs.

use serde::{Deserialize, Serialize};

use crate::id::{GenericId, GuildId, RoleId, UserId};

crate::discord_flags! {
    /// Guild and channel permissions. Discord sends them as a decimal string.
    pub struct Permissions: u64 as string {
        /// Create instant invites.
        const CREATE_INSTANT_INVITE = 1 << 0;
        /// Kick members. Needs 2FA in guilds that enforce it.
        const KICK_MEMBERS = 1 << 1;
        /// Ban members. Needs 2FA in guilds that enforce it.
        const BAN_MEMBERS = 1 << 2;
        /// All permissions, and bypass of channel overwrites. Needs 2FA in guilds that enforce it.
        const ADMINISTRATOR = 1 << 3;
        /// Manage and edit channels. Needs 2FA in guilds that enforce it.
        const MANAGE_CHANNELS = 1 << 4;
        /// Manage and edit the guild. Needs 2FA in guilds that enforce it.
        const MANAGE_GUILD = 1 << 5;
        /// Add new reactions to messages.
        const ADD_REACTIONS = 1 << 6;
        /// View the audit log.
        const VIEW_AUDIT_LOG = 1 << 7;
        /// Use priority speaker in a voice channel.
        const PRIORITY_SPEAKER = 1 << 8;
        /// Go live (stream).
        const STREAM = 1 << 9;
        /// View a channel, read messages, and join voice channels.
        const VIEW_CHANNEL = 1 << 10;
        /// Send messages and create threads in forum channels.
        const SEND_MESSAGES = 1 << 11;
        /// Send text-to-speech messages.
        const SEND_TTS_MESSAGES = 1 << 12;
        /// Delete messages of other users. Needs 2FA in guilds that enforce it.
        const MANAGE_MESSAGES = 1 << 13;
        /// Show embeds for links.
        const EMBED_LINKS = 1 << 14;
        /// Upload files.
        const ATTACH_FILES = 1 << 15;
        /// Read the message history.
        const READ_MESSAGE_HISTORY = 1 << 16;
        /// Mention `@everyone`, `@here`, and all roles.
        const MENTION_EVERYONE = 1 << 17;
        /// Use emojis from other guilds.
        const USE_EXTERNAL_EMOJIS = 1 << 18;
        /// View guild insights.
        const VIEW_GUILD_INSIGHTS = 1 << 19;
        /// Connect to a voice channel.
        const CONNECT = 1 << 20;
        /// Speak in a voice channel.
        const SPEAK = 1 << 21;
        /// Mute members in a voice channel.
        const MUTE_MEMBERS = 1 << 22;
        /// Deafen members in a voice channel.
        const DEAFEN_MEMBERS = 1 << 23;
        /// Move members between voice channels.
        const MOVE_MEMBERS = 1 << 24;
        /// Use voice activity detection.
        const USE_VAD = 1 << 25;
        /// Change the own nickname.
        const CHANGE_NICKNAME = 1 << 26;
        /// Change the nicknames of other members.
        const MANAGE_NICKNAMES = 1 << 27;
        /// Manage roles. Needs 2FA in guilds that enforce it.
        const MANAGE_ROLES = 1 << 28;
        /// Manage webhooks. Needs 2FA in guilds that enforce it.
        const MANAGE_WEBHOOKS = 1 << 29;
        /// Edit and delete emojis, stickers, and soundboard sounds of all users.
        const MANAGE_GUILD_EXPRESSIONS = 1 << 30;
        /// Use application commands and context menu commands.
        const USE_APPLICATION_COMMANDS = 1 << 31;
        /// Request to speak in stage channels.
        const REQUEST_TO_SPEAK = 1 << 32;
        /// Edit and delete scheduled events of all users.
        const MANAGE_EVENTS = 1 << 33;
        /// Delete and archive threads, and view all private threads. Needs 2FA in guilds that enforce it.
        const MANAGE_THREADS = 1 << 34;
        /// Create public and announcement threads.
        const CREATE_PUBLIC_THREADS = 1 << 35;
        /// Create private threads.
        const CREATE_PRIVATE_THREADS = 1 << 36;
        /// Use stickers from other guilds.
        const USE_EXTERNAL_STICKERS = 1 << 37;
        /// Send messages in threads.
        const SEND_MESSAGES_IN_THREADS = 1 << 38;
        /// Use activities (embedded applications) in voice channels.
        const USE_EMBEDDED_ACTIVITIES = 1 << 39;
        /// Time out members.
        const MODERATE_MEMBERS = 1 << 40;
        /// View role subscription insights. Needs 2FA in guilds that enforce it.
        const VIEW_CREATOR_MONETIZATION_ANALYTICS = 1 << 41;
        /// Use the soundboard in a voice channel.
        const USE_SOUNDBOARD = 1 << 42;
        /// Create emojis, stickers, and soundboard sounds, and edit or delete the own ones.
        const CREATE_GUILD_EXPRESSIONS = 1 << 43;
        /// Create scheduled events, and edit or delete the own ones.
        const CREATE_EVENTS = 1 << 44;
        /// Use soundboard sounds from other guilds.
        const USE_EXTERNAL_SOUNDS = 1 << 45;
        /// Send voice messages.
        const SEND_VOICE_MESSAGES = 1 << 46;
        /// Set the status of a voice channel.
        const SET_VOICE_CHANNEL_STATUS = 1 << 48;
        /// Send polls.
        const SEND_POLLS = 1 << 49;
        /// Use user-installed applications in the guild.
        const USE_EXTERNAL_APPS = 1 << 50;
        /// Pin and unpin messages.
        const PIN_MESSAGES = 1 << 51;
        /// Bypass the slowmode of a channel.
        const BYPASS_SLOWMODE = 1 << 52;
    }
}

impl Permissions {
    /// The permissions that a timed-out member keeps.
    pub const TIMED_OUT: Self = Self::VIEW_CHANNEL.union(Self::READ_MESSAGE_HISTORY);

    /// The permissions that a member loses without `SEND_MESSAGES`.
    pub const NEEDS_SEND_MESSAGES: Self = Self::MENTION_EVERYONE
        .union(Self::SEND_TTS_MESSAGES)
        .union(Self::ATTACH_FILES)
        .union(Self::EMBED_LINKS);
}

crate::discord_enum! {
    /// The target kind of a permission overwrite.
    pub enum PermissionOverwriteType: u8 {
        /// The overwrite targets a role.
        Role = 0,
        /// The overwrite targets a member.
        Member = 1,
    }
}

/// A permission overwrite on a channel, for a role or a member.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct PermissionOverwrite {
    /// The role ID or the user ID.
    pub id: GenericId,
    /// The target kind.
    #[serde(rename = "type")]
    pub kind: PermissionOverwriteType,
    /// The permissions that the overwrite allows.
    pub allow: Permissions,
    /// The permissions that the overwrite denies.
    pub deny: Permissions,
}

impl PermissionOverwrite {
    /// Makes an overwrite for a role.
    #[must_use]
    pub const fn role(role_id: RoleId, allow: Permissions, deny: Permissions) -> Self {
        Self {
            id: role_id.cast(),
            kind: PermissionOverwriteType::Role,
            allow,
            deny,
        }
    }

    /// Makes an overwrite for a member.
    #[must_use]
    pub const fn member(user_id: UserId, allow: Permissions, deny: Permissions) -> Self {
        Self {
            id: user_id.cast(),
            kind: PermissionOverwriteType::Member,
            allow,
            deny,
        }
    }
}

/// The kind of channel for the implicit permission rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChannelKind {
    /// A text, announcement, forum, or media channel.
    Text,
    /// A voice or stage channel.
    Voice,
    /// A thread. Give the overwrites of the parent channel.
    Thread,
    /// A category or another channel kind with no implicit rules.
    Other,
}

/// The facts about a member that the permission calculator needs.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct MemberPermissionInput<'a> {
    /// The guild ID. It is also the ID of the `@everyone` role.
    pub guild_id: GuildId,
    /// The user ID of the guild owner.
    pub owner_id: UserId,
    /// The user ID of the member.
    pub user_id: UserId,
    /// The permissions of the `@everyone` role.
    pub everyone_permissions: Permissions,
    /// The roles of the member (without `@everyone`) and their permissions.
    pub roles: &'a [(RoleId, Permissions)],
    /// Tells if the member is timed out now.
    pub timed_out: bool,
}

impl<'a> MemberPermissionInput<'a> {
    /// Makes the input for a member.
    #[must_use]
    pub const fn new(
        guild_id: GuildId,
        owner_id: UserId,
        user_id: UserId,
        everyone_permissions: Permissions,
        roles: &'a [(RoleId, Permissions)],
    ) -> Self {
        Self {
            guild_id,
            owner_id,
            user_id,
            everyone_permissions,
            roles,
            timed_out: false,
        }
    }

    /// Sets whether the member is timed out now.
    #[must_use]
    pub const fn timed_out(mut self, timed_out: bool) -> Self {
        self.timed_out = timed_out;
        self
    }

    /// Computes the guild-level permissions of the member.
    #[must_use]
    pub fn guild_permissions(&self) -> Permissions {
        if self.user_id == self.owner_id {
            return Permissions::all();
        }

        let base = self
            .roles
            .iter()
            .fold(self.everyone_permissions, |total, (_, permissions)| {
                total | *permissions
            });

        if base.contains(Permissions::ADMINISTRATOR) {
            return Permissions::all();
        }

        if self.timed_out {
            return base & Permissions::TIMED_OUT;
        }

        base
    }

    /// Computes the permissions of the member in a channel.
    ///
    /// For a thread, give the overwrites of the parent channel and `ChannelKind::Thread`.
    /// Set `connected_to_voice` when the member is in this voice channel now.
    #[must_use]
    pub fn channel_permissions(
        &self,
        kind: ChannelKind,
        overwrites: &[PermissionOverwrite],
        connected_to_voice: bool,
    ) -> Permissions {
        if self.user_id == self.owner_id {
            return Permissions::all();
        }

        let base = self
            .roles
            .iter()
            .fold(self.everyone_permissions, |total, (_, permissions)| {
                total | *permissions
            });
        if base.contains(Permissions::ADMINISTRATOR) {
            return Permissions::all();
        }

        let mut permissions =
            apply_overwrites(base, self.guild_id, self.user_id, self.roles, overwrites);

        // Being connected to a voice channel also gives the right to view it.
        let can_view = permissions.contains(Permissions::VIEW_CHANNEL)
            || (kind == ChannelKind::Voice && connected_to_voice);
        if !can_view {
            return Permissions::empty();
        }
        permissions.insert(Permissions::VIEW_CHANNEL);

        permissions = apply_implicit_rules(permissions, kind, connected_to_voice);

        if self.timed_out {
            permissions &= Permissions::TIMED_OUT;
        }

        permissions
    }
}

fn apply_overwrites(
    base: Permissions,
    guild_id: GuildId,
    user_id: UserId,
    roles: &[(RoleId, Permissions)],
    overwrites: &[PermissionOverwrite],
) -> Permissions {
    let mut permissions = base;
    let everyone_id: GenericId = guild_id.cast();
    let member_id: GenericId = user_id.cast();

    let everyone = overwrites.iter().find(|overwrite| {
        overwrite.kind == PermissionOverwriteType::Role && overwrite.id == everyone_id
    });
    if let Some(overwrite) = everyone {
        permissions = (permissions - overwrite.deny) | overwrite.allow;
    }

    let mut role_allow = Permissions::empty();
    let mut role_deny = Permissions::empty();
    for overwrite in overwrites {
        let is_member_role = overwrite.kind == PermissionOverwriteType::Role
            && overwrite.id != everyone_id
            && roles
                .iter()
                .any(|(role_id, _)| role_id.cast() == overwrite.id);
        if is_member_role {
            role_allow |= overwrite.allow;
            role_deny |= overwrite.deny;
        }
    }
    permissions = (permissions - role_deny) | role_allow;

    let member = overwrites.iter().find(|overwrite| {
        overwrite.kind == PermissionOverwriteType::Member && overwrite.id == member_id
    });
    if let Some(overwrite) = member {
        permissions = (permissions - overwrite.deny) | overwrite.allow;
    }

    permissions
}

fn apply_implicit_rules(
    mut permissions: Permissions,
    kind: ChannelKind,
    connected_to_voice: bool,
) -> Permissions {
    match kind {
        ChannelKind::Thread => {
            // Threads do not inherit SEND_MESSAGES. SEND_MESSAGES_IN_THREADS takes its place.
            let can_send = permissions.contains(Permissions::SEND_MESSAGES_IN_THREADS);
            permissions.set(Permissions::SEND_MESSAGES, can_send);
        }
        ChannelKind::Voice if !permissions.contains(Permissions::CONNECT) => {
            // The docs say that no CONNECT also denies "other permissions such as MANAGE_CHANNELS".
            // This applies the strict form: only VIEW_CHANNEL stays.
            let keep_status =
                connected_to_voice && permissions.contains(Permissions::SET_VOICE_CHANNEL_STATUS);
            permissions = Permissions::VIEW_CHANNEL;
            permissions.set(Permissions::SET_VOICE_CHANNEL_STATUS, keep_status);
        }
        ChannelKind::Text | ChannelKind::Voice | ChannelKind::Other => {}
    }

    if !permissions.contains(Permissions::SEND_MESSAGES) {
        permissions -= Permissions::NEEDS_SEND_MESSAGES;
    }

    permissions
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUILD: GuildId = GuildId::new_const(100);
    const OWNER: UserId = UserId::new_const(1);
    const USER: UserId = UserId::new_const(2);
    const ROLE_A: RoleId = RoleId::new_const(10);
    const ROLE_B: RoleId = RoleId::new_const(11);

    fn input(roles: &[(RoleId, Permissions)]) -> MemberPermissionInput<'_> {
        let everyone =
            Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::CONNECT;
        MemberPermissionInput::new(GUILD, OWNER, USER, everyone, roles)
    }

    #[test]
    fn serializes_as_string_and_keeps_unknown_bits() {
        let permissions: Permissions = serde_json::from_str("\"9007199254741000\"").unwrap();

        assert_eq!(permissions.bits(), 9_007_199_254_741_000);
        assert_ne!(permissions.unknown_bits(), 0);
        assert_eq!(
            serde_json::to_string(&permissions).unwrap(),
            "\"9007199254741000\""
        );
    }

    #[test]
    fn owner_and_administrator_get_all() {
        let roles = [(ROLE_A, Permissions::ADMINISTRATOR)];
        let owner = MemberPermissionInput::new(GUILD, OWNER, OWNER, Permissions::empty(), &[]);
        let deny_all = [PermissionOverwrite::member(
            USER,
            Permissions::empty(),
            Permissions::all(),
        )];

        assert_eq!(owner.guild_permissions(), Permissions::all());
        assert_eq!(input(&roles).guild_permissions(), Permissions::all());
        assert_eq!(
            input(&roles).channel_permissions(ChannelKind::Text, &deny_all, false),
            Permissions::all()
        );
    }

    #[test]
    fn applies_overwrites_in_order() {
        let roles = [
            (ROLE_A, Permissions::empty()),
            (ROLE_B, Permissions::empty()),
        ];
        let overwrites = [
            PermissionOverwrite::role(
                GUILD.cast(),
                Permissions::empty(),
                Permissions::SEND_MESSAGES,
            ),
            PermissionOverwrite::role(ROLE_A, Permissions::SEND_MESSAGES, Permissions::empty()),
            PermissionOverwrite::role(ROLE_B, Permissions::empty(), Permissions::SEND_MESSAGES),
        ];

        // Role overwrites combine: allow wins over deny between roles.
        let with_roles = input(&roles).channel_permissions(ChannelKind::Text, &overwrites, false);
        assert!(with_roles.contains(Permissions::SEND_MESSAGES));

        let mut with_member = overwrites.to_vec();
        with_member.push(PermissionOverwrite::member(
            USER,
            Permissions::empty(),
            Permissions::SEND_MESSAGES,
        ));
        let denied = input(&roles).channel_permissions(ChannelKind::Text, &with_member, false);
        assert!(!denied.contains(Permissions::SEND_MESSAGES));
    }

    #[test]
    fn no_view_channel_gives_nothing() {
        let overwrites = [PermissionOverwrite::role(
            GUILD.cast(),
            Permissions::empty(),
            Permissions::VIEW_CHANNEL,
        )];

        assert!(
            input(&[])
                .channel_permissions(ChannelKind::Text, &overwrites, false)
                .is_empty()
        );
    }

    #[test]
    fn no_send_messages_removes_dependent_permissions() {
        let roles = [(ROLE_A, Permissions::EMBED_LINKS | Permissions::ATTACH_FILES)];
        let overwrites = [PermissionOverwrite::role(
            GUILD.cast(),
            Permissions::empty(),
            Permissions::SEND_MESSAGES,
        )];

        let permissions = input(&roles).channel_permissions(ChannelKind::Text, &overwrites, false);

        assert!(!permissions.intersects(Permissions::NEEDS_SEND_MESSAGES));
        assert!(permissions.contains(Permissions::VIEW_CHANNEL));
    }

    #[test]
    fn thread_needs_send_messages_in_threads() {
        let without = input(&[]).channel_permissions(ChannelKind::Thread, &[], false);
        let roles = [(ROLE_A, Permissions::SEND_MESSAGES_IN_THREADS)];
        let with = input(&roles).channel_permissions(ChannelKind::Thread, &[], false);

        assert!(!without.contains(Permissions::SEND_MESSAGES));
        assert!(with.contains(Permissions::SEND_MESSAGES));
    }

    #[test]
    fn voice_without_connect_keeps_only_view() {
        let roles = [(
            ROLE_A,
            Permissions::MANAGE_CHANNELS | Permissions::SET_VOICE_CHANNEL_STATUS,
        )];
        let overwrites = [PermissionOverwrite::role(
            GUILD.cast(),
            Permissions::empty(),
            Permissions::CONNECT,
        )];

        let outside = input(&roles).channel_permissions(ChannelKind::Voice, &overwrites, false);
        let connected = input(&roles).channel_permissions(ChannelKind::Voice, &overwrites, true);

        assert_eq!(outside, Permissions::VIEW_CHANNEL);
        assert_eq!(
            connected,
            Permissions::VIEW_CHANNEL | Permissions::SET_VOICE_CHANNEL_STATUS
        );
    }

    #[test]
    fn timeout_keeps_only_view_and_history() {
        let roles = [(
            ROLE_A,
            Permissions::READ_MESSAGE_HISTORY | Permissions::KICK_MEMBERS,
        )];
        let member = input(&roles).timed_out(true);

        assert_eq!(member.guild_permissions(), Permissions::TIMED_OUT);
        assert_eq!(
            member.channel_permissions(ChannelKind::Text, &[], false),
            Permissions::TIMED_OUT
        );
    }
}
