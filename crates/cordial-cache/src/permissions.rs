//! Permission helpers that use the cached guild, roles, members, and channels.

use cordial_model::id::{ChannelId, GuildId, RoleId, UserId};
use cordial_model::permissions::{ChannelKind, MemberPermissionInput};
use cordial_model::{Permissions, Timestamp};

use crate::Cache;

/// The reason the cache cannot compute permissions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PermissionError {
    /// The cache has no such guild.
    MissingGuild(GuildId),
    /// The cache has no such channel.
    MissingChannel(ChannelId),
    /// The channel is not in a guild.
    NotGuildChannel(ChannelId),
    /// The cache has no such member.
    MissingMember(UserId),
    /// The cache has no `@everyone` role for the guild.
    MissingEveryoneRole(GuildId),
    /// Discord obfuscated the channel, because the bot cannot view it. Its overwrites are
    /// not known.
    Obfuscated(ChannelId),
}

impl std::fmt::Display for PermissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingGuild(id) => write!(formatter, "the cache has no guild {id}"),
            Self::MissingChannel(id) => write!(formatter, "the cache has no channel {id}"),
            Self::NotGuildChannel(id) => write!(formatter, "channel {id} is not in a guild"),
            Self::MissingMember(id) => write!(formatter, "the cache has no member {id}"),
            Self::MissingEveryoneRole(id) => {
                write!(formatter, "the cache has no @everyone role for guild {id}")
            }
            Self::Obfuscated(id) => write!(
                formatter,
                "channel {id} is obfuscated; the bot cannot view it"
            ),
        }
    }
}

impl std::error::Error for PermissionError {}

/// The cached facts that the permission calculation needs.
struct MemberFacts {
    owner_id: UserId,
    everyone: Permissions,
    roles: Vec<(RoleId, Permissions)>,
    timed_out: bool,
}

impl MemberFacts {
    fn input(&self, guild_id: GuildId, user_id: UserId) -> MemberPermissionInput<'_> {
        MemberPermissionInput::new(guild_id, self.owner_id, user_id, self.everyone, &self.roles)
            .timed_out(self.timed_out)
    }
}

impl Cache {
    /// Computes the guild-level permissions of a member.
    ///
    /// # Errors
    ///
    /// Gives an error when the guild, the member, or the `@everyone` role is not cached.
    pub fn guild_permissions(
        &self,
        guild_id: GuildId,
        user_id: UserId,
    ) -> Result<Permissions, PermissionError> {
        let facts = self.member_facts(guild_id, user_id)?;
        let input = facts.input(guild_id, user_id);
        Ok(input.guild_permissions())
    }

    /// Computes the permissions of a member in a channel or a thread.
    ///
    /// For a thread, the overwrites of the parent channel apply.
    ///
    /// # Errors
    ///
    /// Gives an error when a needed object is not cached, or when the channel is obfuscated.
    pub fn channel_permissions(
        &self,
        channel_id: ChannelId,
        user_id: UserId,
    ) -> Result<Permissions, PermissionError> {
        let channel = self
            .channel(channel_id)
            .ok_or(PermissionError::MissingChannel(channel_id))?;
        let guild_id = channel
            .guild_id
            .ok_or(PermissionError::NotGuildChannel(channel_id))?;
        if channel.is_obfuscated() {
            return Err(PermissionError::Obfuscated(channel_id));
        }

        let kind = channel.kind.permission_kind();
        let overwrite_channel = if kind == ChannelKind::Thread {
            let parent_id = channel
                .parent_id
                .ok_or(PermissionError::MissingChannel(channel_id))?;
            let parent = self
                .channel(parent_id)
                .ok_or(PermissionError::MissingChannel(parent_id))?;
            if parent.is_obfuscated() {
                return Err(PermissionError::Obfuscated(parent_id));
            }
            parent
        } else {
            channel
        };
        let overwrites = overwrite_channel
            .permission_overwrites
            .clone()
            .unwrap_or_default();

        let connected = self
            .voice_state(guild_id, user_id)
            .is_some_and(|state| state.channel_id == Some(channel_id));
        let facts = self.member_facts(guild_id, user_id)?;
        let input = facts.input(guild_id, user_id);
        Ok(input.channel_permissions(kind, &overwrites, connected))
    }

    /// Gives the owner, the `@everyone` permissions, the member roles, and the timeout state.
    fn member_facts(
        &self,
        guild_id: GuildId,
        user_id: UserId,
    ) -> Result<MemberFacts, PermissionError> {
        let guild = self
            .guild(guild_id)
            .ok_or(PermissionError::MissingGuild(guild_id))?;
        let everyone_id: RoleId = guild_id.cast();
        let everyone = self
            .role(everyone_id)
            .ok_or(PermissionError::MissingEveryoneRole(guild_id))?
            .permissions;

        if user_id == guild.owner_id {
            return Ok(MemberFacts {
                owner_id: guild.owner_id,
                everyone,
                roles: Vec::new(),
                timed_out: false,
            });
        }

        let member = self
            .member(guild_id, user_id)
            .ok_or(PermissionError::MissingMember(user_id))?;
        let roles = member
            .roles
            .iter()
            .filter_map(|role_id| self.role(*role_id).map(|role| (*role_id, role.permissions)))
            .collect();
        let timed_out = member.is_timed_out_at(Timestamp::now());
        Ok(MemberFacts {
            owner_id: guild.owner_id,
            everyone,
            roles,
            timed_out,
        })
    }
}
