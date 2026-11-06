//! The update of the cache from gateway events.

use std::collections::HashSet;
use std::hash::Hash;
use std::sync::Arc;

use cordial_model::channel::Channel;
use cordial_model::gateway::event::{Event, GuildCreate};
use cordial_model::gateway::presence::Presence;
use cordial_model::guild::{Guild, Member, Role};
use cordial_model::id::{GuildId, UserId};
use cordial_model::message::Message;
use cordial_model::user::User;
use cordial_model::voice::VoiceState;
use dashmap::DashMap;

use crate::model::{CachedGuild, CachedMember, CachedRole};
use crate::{Cache, ResourceTypes};

/// The value of an object before an event changed or removed it.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Previous {
    /// A guild.
    Guild(Arc<CachedGuild>),
    /// A channel or a thread.
    Channel(Arc<Channel>),
    /// A role.
    Role(Arc<CachedRole>),
    /// A member.
    Member(Arc<CachedMember>),
    /// A user.
    User(Arc<User>),
    /// A voice state.
    VoiceState(Arc<VoiceState>),
    /// A presence.
    Presence(Arc<Presence>),
    /// A message.
    Message(Arc<Message>),
    /// Messages of a bulk delete.
    Messages(Vec<Arc<Message>>),
}

fn index_insert<K: Eq + Hash>(index: &DashMap<GuildId, HashSet<K>>, guild_id: GuildId, key: K) {
    index.entry(guild_id).or_default().insert(key);
}

fn index_remove<K: Eq + Hash>(index: &DashMap<GuildId, HashSet<K>>, guild_id: GuildId, key: &K) {
    if let Some(mut set) = index.get_mut(&guild_id) {
        set.remove(key);
    }
}

impl Cache {
    /// Updates the cache from a gateway event. Gives the previous value of the changed or
    /// removed object, when the cache had it.
    pub fn update(&self, event: &Event) -> Option<Previous> {
        match event {
            Event::Ready(ready) => {
                self.set_current_user(&ready.user);
                for guild in &ready.guilds {
                    self.inner.unavailable_guilds.insert(guild.id, ());
                }
                None
            }
            Event::GuildCreate(created) => match &**created {
                GuildCreate::Available(guild) => self.insert_guild(guild),
                GuildCreate::Unavailable(guild) => {
                    self.inner.unavailable_guilds.insert(guild.id, ());
                    None
                }
                _ => None,
            },
            Event::GuildUpdate(updated) => self.update_guild(updated),
            Event::GuildDelete(deleted) => self.remove_guild(deleted.id, deleted.unavailable),

            Event::ChannelCreate(channel) => self.insert_channel(&channel.0),
            Event::ThreadCreate(channel) => self.insert_channel(&channel.0),
            Event::ChannelUpdate(channel) => self.insert_channel(&channel.0),
            Event::ThreadUpdate(channel) => self.insert_channel(&channel.0),
            Event::ChannelDelete(channel) => self.remove_channel(&channel.0),
            Event::ThreadDelete(channel) => self.remove_channel(&channel.0),
            Event::ThreadListSync(sync) => {
                for thread in &sync.threads {
                    self.insert_channel_with_guild(thread, Some(sync.guild_id));
                }
                None
            }
            Event::ChannelPinsUpdate(pins) => {
                if let Some(mut channel) = self.inner.channels.get_mut(&pins.channel_id) {
                    Arc::make_mut(&mut channel).last_pin_timestamp = pins.last_pin_timestamp;
                }
                None
            }

            Event::GuildRoleCreate(created) => self.insert_role(created.guild_id, &created.role),
            Event::GuildRoleUpdate(updated) => self.insert_role(updated.guild_id, &updated.role),
            Event::GuildRoleDelete(deleted) => {
                index_remove(&self.inner.guild_roles, deleted.guild_id, &deleted.role_id);
                self.inner
                    .roles
                    .remove(&deleted.role_id)
                    .map(|(_, role)| Previous::Role(role))
            }

            Event::GuildMemberAdd(added) => {
                let guild_id = added.guild_id?;
                self.adjust_member_count(guild_id, 1);
                self.insert_member(guild_id, &added.0)
            }
            Event::GuildMemberUpdate(update) => {
                if !self.add_guild_user(update.guild_id, &update.user) {
                    return None;
                }
                if !self.wants(ResourceTypes::MEMBER) {
                    return None;
                }
                let key = (update.guild_id, update.user.id);
                let previous = crate::get(&self.inner.members, &key);
                let mut member = previous
                    .as_deref()
                    .cloned()
                    .unwrap_or_else(|| CachedMember {
                        guild_id: update.guild_id,
                        user_id: update.user.id,
                        nick: None,
                        avatar: None,
                        banner: None,
                        roles: Vec::new(),
                        joined_at: None,
                        premium_since: None,
                        deaf: false,
                        mute: false,
                        flags: cordial_model::guild::MemberFlags::empty(),
                        pending: false,
                        communication_disabled_until: None,
                        avatar_decoration_data: None,
                        collectibles: None,
                    });
                member.apply_update(update);
                self.store_member(member);
                previous.map(Previous::Member)
            }
            Event::GuildMemberRemove(removed) => {
                self.adjust_member_count(removed.guild_id, -1);
                self.remove_member(removed.guild_id, removed.user.id)
            }
            Event::GuildMembersChunk(chunk) => {
                for member in &chunk.members {
                    self.insert_member(chunk.guild_id, member);
                }
                if let Some(presences) = &chunk.presences {
                    for presence in presences {
                        self.insert_presence(chunk.guild_id, presence);
                    }
                }
                None
            }
            Event::UserUpdate(user) => {
                self.set_current_user(user);
                None
            }

            Event::GuildEmojisUpdate(update) if self.wants(ResourceTypes::EMOJI) => {
                self.replace_emojis(update.guild_id, &update.emojis);
                None
            }
            Event::GuildStickersUpdate(update) if self.wants(ResourceTypes::STICKER) => {
                replace_all(
                    &self.inner.stickers,
                    &self.inner.guild_stickers,
                    update.guild_id,
                    update
                        .stickers
                        .iter()
                        .map(|sticker| (sticker.id, sticker.clone())),
                );
                None
            }

            Event::VoiceStateUpdate(state) => self.insert_voice_state(state.guild_id, state),
            Event::PresenceUpdate(presence) => {
                let guild_id = presence.guild_id?;
                self.insert_presence(guild_id, presence)
            }

            Event::MessageCreate(message) => {
                // A webhook author is not a real user and has no member.
                if let (Some(guild_id), Some(member), None) =
                    (message.guild_id, &message.member, message.webhook_id)
                    && self.add_guild_user(guild_id, &message.author)
                    && self.wants(ResourceTypes::MEMBER)
                {
                    let cached = CachedMember::from_partial(guild_id, message.author.id, member);
                    self.store_member(cached);
                } else {
                    self.refresh_user(&message.author);
                }
                self.insert_message(message);
                None
            }
            Event::MessageUpdate(message) => self.update_message(message),
            Event::MessageDelete(deleted) => {
                self.remove_messages(deleted.channel_id, &[deleted.id])
            }
            Event::MessageDeleteBulk(deleted) => {
                self.remove_messages(deleted.channel_id, &deleted.ids)
            }
            Event::MessageReactionAdd(reaction) => {
                let me = self.is_current_user(reaction.user_id);
                self.edit_message(reaction.channel_id, reaction.message_id, |message| {
                    message.add_reaction(&reaction.emoji, reaction.burst, me);
                });
                None
            }
            Event::MessageReactionRemove(reaction) => {
                let me = self.is_current_user(reaction.user_id);
                self.edit_message(reaction.channel_id, reaction.message_id, |message| {
                    message.remove_reaction(&reaction.emoji, reaction.burst, me);
                });
                None
            }
            Event::MessageReactionRemoveAll(removed) => {
                self.edit_message(removed.channel_id, removed.message_id, |message| {
                    message.reactions = None;
                });
                None
            }
            Event::MessageReactionRemoveEmoji(removed) => {
                self.edit_message(removed.channel_id, removed.message_id, |message| {
                    message.remove_reaction_emoji(&removed.emoji);
                });
                None
            }

            Event::StageInstanceCreate(stage) => {
                self.insert_stage(&stage.0);
                None
            }
            Event::StageInstanceUpdate(stage) => {
                self.insert_stage(&stage.0);
                None
            }
            Event::StageInstanceDelete(stage) => {
                index_remove(&self.inner.guild_stage_instances, stage.guild_id, &stage.id);
                self.inner.stage_instances.remove(&stage.id);
                None
            }

            Event::GuildScheduledEventCreate(scheduled) => {
                self.insert_scheduled_event(&scheduled.0);
                None
            }
            Event::GuildScheduledEventUpdate(scheduled) => {
                self.insert_scheduled_event(&scheduled.0);
                None
            }
            Event::GuildScheduledEventDelete(scheduled) => {
                index_remove(
                    &self.inner.guild_scheduled_events,
                    scheduled.guild_id,
                    &scheduled.id,
                );
                self.inner.scheduled_events.remove(&scheduled.id);
                None
            }

            Event::GuildSoundboardSoundCreate(sound) => {
                self.insert_sound(&sound.0);
                None
            }
            Event::GuildSoundboardSoundUpdate(sound) => {
                self.insert_sound(&sound.0);
                None
            }
            Event::GuildSoundboardSoundDelete(deleted) => {
                index_remove(
                    &self.inner.guild_soundboard_sounds,
                    deleted.guild_id,
                    &deleted.sound_id,
                );
                self.inner.soundboard_sounds.remove(&deleted.sound_id);
                None
            }
            Event::GuildSoundboardSoundsUpdate(update)
                if self.wants(ResourceTypes::SOUNDBOARD_SOUND) =>
            {
                self.replace_sounds(update.guild_id, &update.soundboard_sounds);
                None
            }
            Event::SoundboardSounds(sounds) if self.wants(ResourceTypes::SOUNDBOARD_SOUND) => {
                self.replace_sounds(sounds.guild_id, &sounds.soundboard_sounds);
                None
            }

            _ => None,
        }
    }

    fn insert_stage(&self, stage: &cordial_model::voice::StageInstance) {
        if self.wants(ResourceTypes::STAGE_INSTANCE) {
            index_insert(&self.inner.guild_stage_instances, stage.guild_id, stage.id);
            self.inner
                .stage_instances
                .insert(stage.id, Arc::new(stage.clone()));
        }
    }

    fn insert_scheduled_event(&self, scheduled: &cordial_model::scheduled_event::ScheduledEvent) {
        if self.wants(ResourceTypes::SCHEDULED_EVENT) {
            index_insert(
                &self.inner.guild_scheduled_events,
                scheduled.guild_id,
                scheduled.id,
            );
            self.inner
                .scheduled_events
                .insert(scheduled.id, Arc::new(scheduled.clone()));
        }
    }

    fn insert_sound(&self, sound: &cordial_model::soundboard::SoundboardSound) {
        if self.wants(ResourceTypes::SOUNDBOARD_SOUND) {
            if let Some(guild_id) = sound.guild_id {
                index_insert(
                    &self.inner.guild_soundboard_sounds,
                    guild_id,
                    sound.sound_id,
                );
            }
            self.inner
                .soundboard_sounds
                .insert(sound.sound_id, Arc::new(sound.clone()));
        }
    }

    fn set_current_user(&self, user: &User) {
        let user = Arc::new(user.clone());
        *self
            .inner
            .current_user
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(Arc::clone(&user));
        if self.wants(ResourceTypes::USER) {
            self.inner.users.insert(user.id, user);
        }
    }

    fn insert_guild(&self, guild: &Guild) -> Option<Previous> {
        let guild_id = guild.id;
        self.inner.unavailable_guilds.remove(&guild_id);
        let previous = if self.wants(ResourceTypes::GUILD) {
            self.inner
                .guilds
                .insert(guild_id, Arc::new(CachedGuild::new(guild.clone())))
        } else {
            None
        };

        if let Some(channels) = &guild.channels {
            for channel in channels {
                self.insert_channel_with_guild(channel, Some(guild_id));
            }
        }
        if let Some(threads) = &guild.threads {
            for thread in threads {
                self.insert_channel_with_guild(thread, Some(guild_id));
            }
        }
        for role in &guild.roles {
            self.insert_role(guild_id, role);
        }
        if let Some(members) = &guild.members {
            for member in members {
                self.insert_member(guild_id, member);
            }
        }
        if self.wants(ResourceTypes::EMOJI) {
            self.replace_emojis(guild_id, &guild.emojis);
        }
        if let (true, Some(stickers)) = (self.wants(ResourceTypes::STICKER), &guild.stickers) {
            replace_all(
                &self.inner.stickers,
                &self.inner.guild_stickers,
                guild_id,
                stickers.iter().map(|sticker| (sticker.id, sticker.clone())),
            );
        }
        if let Some(states) = &guild.voice_states {
            for state in states {
                self.insert_voice_state(Some(guild_id), state);
            }
        }
        if let Some(presences) = &guild.presences {
            for presence in presences {
                self.insert_presence(guild_id, presence);
            }
        }
        if let (true, Some(stages)) = (
            self.wants(ResourceTypes::STAGE_INSTANCE),
            &guild.stage_instances,
        ) {
            for stage in stages {
                index_insert(&self.inner.guild_stage_instances, guild_id, stage.id);
                self.inner
                    .stage_instances
                    .insert(stage.id, Arc::new(stage.clone()));
            }
        }
        if let (true, Some(events)) = (
            self.wants(ResourceTypes::SCHEDULED_EVENT),
            &guild.guild_scheduled_events,
        ) {
            for scheduled in events {
                index_insert(&self.inner.guild_scheduled_events, guild_id, scheduled.id);
                self.inner
                    .scheduled_events
                    .insert(scheduled.id, Arc::new(scheduled.clone()));
            }
        }
        if let (true, Some(sounds)) = (
            self.wants(ResourceTypes::SOUNDBOARD_SOUND),
            &guild.soundboard_sounds,
        ) {
            self.replace_sounds(guild_id, sounds);
        }

        previous.map(Previous::Guild)
    }

    fn update_guild(&self, guild: &Guild) -> Option<Previous> {
        for role in &guild.roles {
            self.insert_role(guild.id, role);
        }
        if self.wants(ResourceTypes::EMOJI) && !guild.emojis.is_empty() {
            self.replace_emojis(guild.id, &guild.emojis);
        }
        if !self.wants(ResourceTypes::GUILD) {
            return None;
        }

        let previous = crate::get(&self.inner.guilds, &guild.id);
        let mut updated = guild.clone();
        // GUILD_UPDATE has no gateway-only fields. Keep them from the cached guild.
        if let Some(old) = &previous {
            updated.joined_at = old.joined_at;
            updated.large = old.large;
            updated.member_count = old.member_count;
        }
        self.inner
            .guilds
            .insert(guild.id, Arc::new(CachedGuild::new(updated)));
        previous.map(Previous::Guild)
    }

    fn adjust_member_count(&self, guild_id: GuildId, change: i64) {
        if let Some(mut guild) = self.inner.guilds.get_mut(&guild_id) {
            let cached = Arc::make_mut(&mut guild);
            let count = cached.0.member_count.unwrap_or(0);
            cached.0.member_count = Some(count.saturating_add_signed(change));
        }
    }

    fn remove_guild(&self, guild_id: GuildId, unavailable: bool) -> Option<Previous> {
        if unavailable {
            self.inner.unavailable_guilds.insert(guild_id, ());
        } else {
            self.inner.unavailable_guilds.remove(&guild_id);
        }

        remove_indexed(&self.inner.guild_channels, guild_id, |channel_id| {
            self.inner.channels.remove(channel_id);
            self.inner.messages.remove(channel_id);
        });
        remove_indexed(&self.inner.guild_roles, guild_id, |role_id| {
            self.inner.roles.remove(role_id);
        });
        remove_indexed(&self.inner.guild_members, guild_id, |user_id| {
            self.inner.members.remove(&(guild_id, *user_id));
            self.inner.presences.remove(&(guild_id, *user_id));
            self.inner.voice_states.remove(&(guild_id, *user_id));
            self.release_user(*user_id, guild_id);
        });
        remove_indexed(&self.inner.guild_emojis, guild_id, |emoji_id| {
            self.inner.emojis.remove(emoji_id);
        });
        remove_indexed(&self.inner.guild_stickers, guild_id, |sticker_id| {
            self.inner.stickers.remove(sticker_id);
        });
        remove_indexed(&self.inner.guild_stage_instances, guild_id, |stage_id| {
            self.inner.stage_instances.remove(stage_id);
        });
        remove_indexed(&self.inner.guild_scheduled_events, guild_id, |event_id| {
            self.inner.scheduled_events.remove(event_id);
        });
        remove_indexed(&self.inner.guild_soundboard_sounds, guild_id, |sound_id| {
            self.inner.soundboard_sounds.remove(sound_id);
        });
        self.inner
            .voice_states
            .retain(|(guild, _), _| *guild != guild_id);
        self.inner
            .presences
            .retain(|(guild, _), _| *guild != guild_id);

        self.inner
            .guilds
            .remove(&guild_id)
            .map(|(_, guild)| Previous::Guild(guild))
    }

    fn insert_channel(&self, channel: &Channel) -> Option<Previous> {
        self.insert_channel_with_guild(channel, channel.guild_id)
    }

    fn insert_channel_with_guild(
        &self,
        channel: &Channel,
        guild_id: Option<GuildId>,
    ) -> Option<Previous> {
        if !self.wants(ResourceTypes::CHANNEL) {
            return None;
        }
        let mut channel = channel.clone();
        if channel.guild_id.is_none() {
            channel.guild_id = guild_id;
        }
        if let Some(guild_id) = channel.guild_id {
            index_insert(&self.inner.guild_channels, guild_id, channel.id);
        }
        self.inner
            .channels
            .insert(channel.id, Arc::new(channel))
            .map(Previous::Channel)
    }

    fn remove_channel(&self, channel: &Channel) -> Option<Previous> {
        if let Some(guild_id) = channel.guild_id {
            index_remove(&self.inner.guild_channels, guild_id, &channel.id);
        }
        self.inner.messages.remove(&channel.id);
        self.inner
            .channels
            .remove(&channel.id)
            .map(|(_, channel)| Previous::Channel(channel))
    }

    fn insert_role(&self, guild_id: GuildId, role: &Role) -> Option<Previous> {
        if !self.wants(ResourceTypes::ROLE) {
            return None;
        }
        index_insert(&self.inner.guild_roles, guild_id, role.id);
        let cached = CachedRole {
            guild_id,
            role: role.clone(),
        };
        self.inner
            .roles
            .insert(role.id, Arc::new(cached))
            .map(Previous::Role)
    }

    fn insert_member(&self, guild_id: GuildId, member: &Member) -> Option<Previous> {
        let user = member.user.as_ref()?;
        if !self.add_guild_user(guild_id, user) || !self.wants(ResourceTypes::MEMBER) {
            return None;
        }
        self.store_member(CachedMember::from_member(guild_id, user.id, member))
            .map(Previous::Member)
    }

    fn store_member(&self, member: CachedMember) -> Option<Arc<CachedMember>> {
        self.inner
            .members
            .insert((member.guild_id, member.user_id), Arc::new(member))
    }

    /// Records that a user is in a guild, and stores the user.
    ///
    /// Gives `false` when the cache keeps guilds but not this guild. Then nothing removes the
    /// data later, so the cache must not store it.
    fn add_guild_user(&self, guild_id: GuildId, user: &User) -> bool {
        if !self.track_guild_user(guild_id, user.id) {
            return false;
        }
        self.inner
            .user_guilds
            .entry(user.id)
            .or_default()
            .insert(guild_id);
        self.upsert_user(user);
        true
    }

    /// Adds a user to the member index of a guild, so that the guild removal cleans up the
    /// member, presence, and voice state of the user.
    ///
    /// Gives `false` when the cache keeps guilds but not this guild.
    fn track_guild_user(&self, guild_id: GuildId, user_id: UserId) -> bool {
        if self.wants(ResourceTypes::GUILD) && !self.inner.guilds.contains_key(&guild_id) {
            return false;
        }
        index_insert(&self.inner.guild_members, guild_id, user_id);
        true
    }

    /// Replaces a user that the cache already has. It does not add a new user.
    fn refresh_user(&self, user: &User) {
        if self.inner.users.contains_key(&user.id) {
            self.upsert_user(user);
        }
    }

    fn remove_member(&self, guild_id: GuildId, user_id: UserId) -> Option<Previous> {
        index_remove(&self.inner.guild_members, guild_id, &user_id);
        self.inner.presences.remove(&(guild_id, user_id));
        let removed = self.inner.members.remove(&(guild_id, user_id));
        self.release_user(user_id, guild_id);
        removed.map(|(_, member)| Previous::Member(member))
    }

    fn upsert_user(&self, user: &User) {
        if !self.wants(ResourceTypes::USER) {
            return;
        }
        let changed = self
            .inner
            .users
            .get(&user.id)
            .is_none_or(|current| **current != *user);
        if changed {
            self.inner.users.insert(user.id, Arc::new(user.clone()));
        }
    }

    /// Removes a user when no guild of the cache has the user as a member any more.
    fn release_user(&self, user_id: UserId, guild_id: GuildId) {
        let last = match self.inner.user_guilds.get_mut(&user_id) {
            Some(mut guilds) => {
                guilds.remove(&guild_id);
                guilds.is_empty()
            }
            None => true,
        };
        if !last {
            return;
        }
        self.inner.user_guilds.remove(&user_id);
        let is_current_user = self
            .current_user()
            .is_some_and(|current| current.id == user_id);
        if !is_current_user {
            self.inner.users.remove(&user_id);
        }
    }

    fn insert_voice_state(
        &self,
        guild_id: Option<GuildId>,
        state: &VoiceState,
    ) -> Option<Previous> {
        if !self.wants(ResourceTypes::VOICE_STATE) {
            return None;
        }
        let guild_id = state.guild_id.or(guild_id)?;
        if !self.track_guild_user(guild_id, state.user_id) {
            return None;
        }
        if let Some(member) = &state.member {
            self.insert_member(guild_id, member);
        }

        let key = (guild_id, state.user_id);
        let previous = if state.channel_id.is_some() {
            self.inner.voice_states.insert(key, Arc::new(state.clone()))
        } else {
            self.inner.voice_states.remove(&key).map(|(_, state)| state)
        };
        previous.map(Previous::VoiceState)
    }

    fn insert_presence(&self, guild_id: GuildId, presence: &Presence) -> Option<Previous> {
        if !self.wants(ResourceTypes::PRESENCE) {
            return None;
        }
        if !self.track_guild_user(guild_id, presence.user.id) {
            return None;
        }
        let key = (guild_id, presence.user.id);
        let offline = presence.status == cordial_model::gateway::presence::Status::Offline;
        let previous = if offline {
            self.inner
                .presences
                .remove(&key)
                .map(|(_, presence)| presence)
        } else {
            self.inner.presences.insert(key, Arc::new(presence.clone()))
        };
        previous.map(Previous::Presence)
    }

    fn replace_emojis(&self, guild_id: GuildId, emojis: &[cordial_model::emoji::Emoji]) {
        remove_indexed(&self.inner.guild_emojis, guild_id, |emoji_id| {
            self.inner.emojis.remove(emoji_id);
        });
        for emoji in emojis {
            if let Some(id) = emoji.id {
                index_insert(&self.inner.guild_emojis, guild_id, id);
                self.inner
                    .emojis
                    .insert(id, (guild_id, Arc::new(emoji.clone())));
            }
        }
    }

    fn replace_sounds(
        &self,
        guild_id: GuildId,
        sounds: &[cordial_model::soundboard::SoundboardSound],
    ) {
        replace_all(
            &self.inner.soundboard_sounds,
            &self.inner.guild_soundboard_sounds,
            guild_id,
            sounds.iter().map(|sound| (sound.sound_id, sound.clone())),
        );
    }

    fn insert_message(&self, message: &Message) {
        if !self.wants(ResourceTypes::MESSAGE) || self.inner.message_capacity == 0 {
            return;
        }
        let mut messages = self.inner.messages.entry(message.channel_id).or_default();
        if messages.len() >= self.inner.message_capacity {
            messages.pop_front();
        }
        messages.push_back(Arc::new(message.clone()));
    }

    fn update_message(&self, message: &Message) -> Option<Previous> {
        if !self.wants(ResourceTypes::MESSAGE) {
            return None;
        }
        let mut messages = self.inner.messages.get_mut(&message.channel_id)?;
        let slot = messages.iter_mut().find(|cached| cached.id == message.id)?;
        let previous = std::mem::replace(slot, Arc::new(message.clone()));
        Some(Previous::Message(previous))
    }

    /// Changes a cached message in place. Other holders of the old `Arc` keep the old value.
    fn edit_message(
        &self,
        channel_id: cordial_model::id::ChannelId,
        message_id: cordial_model::id::MessageId,
        edit: impl FnOnce(&mut Message),
    ) {
        let Some(mut messages) = self.inner.messages.get_mut(&channel_id) else {
            return;
        };
        if let Some(slot) = messages.iter_mut().find(|cached| cached.id == message_id) {
            edit(Arc::make_mut(slot));
        }
    }

    fn is_current_user(&self, user_id: cordial_model::id::UserId) -> bool {
        self.current_user().is_some_and(|user| user.id == user_id)
    }

    fn remove_messages(
        &self,
        channel_id: cordial_model::id::ChannelId,
        ids: &[cordial_model::id::MessageId],
    ) -> Option<Previous> {
        let mut messages = self.inner.messages.get_mut(&channel_id)?;
        let mut removed = Vec::new();
        messages.retain(|message| {
            let delete = ids.contains(&message.id);
            if delete {
                removed.push(Arc::clone(message));
            }
            !delete
        });
        match removed.len() {
            0 => None,
            1 if ids.len() == 1 => removed.pop().map(Previous::Message),
            _ => Some(Previous::Messages(removed)),
        }
    }
}

/// Removes all keys of a guild from an index and runs `remove` for each.
fn remove_indexed<K: Eq + Hash>(
    index: &DashMap<GuildId, HashSet<K>>,
    guild_id: GuildId,
    mut remove: impl FnMut(&K),
) {
    if let Some((_, keys)) = index.remove(&guild_id) {
        for key in &keys {
            remove(key);
        }
    }
}

/// Replaces all values of a guild in a map with new values.
fn replace_all<K: Copy + Eq + Hash, V>(
    values: &DashMap<K, Arc<V>>,
    index: &DashMap<GuildId, HashSet<K>>,
    guild_id: GuildId,
    new_values: impl Iterator<Item = (K, V)>,
) {
    remove_indexed(index, guild_id, |key| {
        values.remove(key);
    });
    for (key, value) in new_values {
        index_insert(index, guild_id, key);
        values.insert(key, Arc::new(value));
    }
}
