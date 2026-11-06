//! In-memory cache of Discord state.
//!
//! The cache takes gateway events as input and keeps the selected resource types.
//! See `docs/ARCHITECTURE.md`, section 8.
//!
//! - Reads give `Arc` values. A read never holds a lock, so it is safe across `.await`.
//! - Members are stored without their user. Users are stored once and shared.
//! - [`Cache::update`] gives the previous value of a changed object.
//!
//! ```
//! use cordial_cache::{Cache, ResourceTypes};
//!
//! let cache = Cache::new(ResourceTypes::GUILD | ResourceTypes::CHANNEL | ResourceTypes::ROLE);
//! // For each gateway event: `cache.update(&event);`
//! assert_eq!(cache.guild_count(), 0);
//! ```

mod model;
mod permissions;
mod update;

use std::collections::{HashSet, VecDeque};
use std::hash::Hash;
use std::sync::{Arc, RwLock};

use cordial_model::channel::Channel;
use cordial_model::emoji::Emoji;
use cordial_model::gateway::presence::Presence;
use cordial_model::gateway::{EventType, EventTypes, Intents};
use cordial_model::id::{
    ChannelId, EmojiId, GuildId, MessageId, RoleId, ScheduledEventId, SoundboardSoundId,
    StageInstanceId, StickerId, UserId,
};
use cordial_model::message::Message;
use cordial_model::scheduled_event::ScheduledEvent;
use cordial_model::soundboard::SoundboardSound;
use cordial_model::sticker::Sticker;
use cordial_model::user::User;
use cordial_model::voice::{StageInstance, VoiceState};
use dashmap::DashMap;

pub use model::{CachedGuild, CachedMember, CachedRole};
pub use permissions::PermissionError;
pub use update::Previous;

cordial_model::discord_flags! {
    /// The resource types that the cache keeps.
    pub struct ResourceTypes: u64 as integer {
        /// Guilds.
        const GUILD = 1 << 0;
        /// Channels and threads.
        const CHANNEL = 1 << 1;
        /// Roles.
        const ROLE = 1 << 2;
        /// Members. Needs the privileged `GUILD_MEMBERS` intent for most member events.
        const MEMBER = 1 << 3;
        /// Users (of members, message authors, and the current user).
        const USER = 1 << 4;
        /// Emojis.
        const EMOJI = 1 << 5;
        /// Stickers.
        const STICKER = 1 << 6;
        /// Voice states.
        const VOICE_STATE = 1 << 7;
        /// Presences. Needs the privileged `GUILD_PRESENCES` intent.
        const PRESENCE = 1 << 8;
        /// Recent messages of each channel. Set the size with [`Cache::with_message_capacity`].
        const MESSAGE = 1 << 9;
        /// Stage instances.
        const STAGE_INSTANCE = 1 << 10;
        /// Scheduled events.
        const SCHEDULED_EVENT = 1 << 11;
        /// Soundboard sounds.
        const SOUNDBOARD_SOUND = 1 << 12;
    }
}

impl ResourceTypes {
    /// Gives the event types that the cache needs for these resources.
    #[must_use]
    pub fn event_types(self) -> EventTypes {
        let mut kinds = vec![
            EventType::Ready,
            EventType::GuildCreate,
            EventType::GuildDelete,
        ];
        if self.contains(Self::GUILD) {
            kinds.extend([EventType::GuildUpdate]);
        }
        if self.contains(Self::CHANNEL) {
            kinds.extend([
                EventType::ChannelCreate,
                EventType::ChannelUpdate,
                EventType::ChannelDelete,
                EventType::ChannelPinsUpdate,
                EventType::ThreadCreate,
                EventType::ThreadUpdate,
                EventType::ThreadDelete,
                EventType::ThreadListSync,
            ]);
        }
        if self.contains(Self::ROLE) {
            kinds.extend([
                EventType::GuildRoleCreate,
                EventType::GuildRoleUpdate,
                EventType::GuildRoleDelete,
            ]);
        }
        if self.intersects(Self::MEMBER | Self::USER) {
            kinds.extend([
                EventType::GuildMemberAdd,
                EventType::GuildMemberUpdate,
                EventType::GuildMemberRemove,
                EventType::GuildMembersChunk,
            ]);
        }
        if self.contains(Self::USER) {
            kinds.push(EventType::UserUpdate);
        }
        if self.contains(Self::EMOJI) {
            kinds.push(EventType::GuildEmojisUpdate);
        }
        if self.contains(Self::STICKER) {
            kinds.push(EventType::GuildStickersUpdate);
        }
        if self.contains(Self::VOICE_STATE) {
            kinds.push(EventType::VoiceStateUpdate);
        }
        if self.contains(Self::PRESENCE) {
            kinds.push(EventType::PresenceUpdate);
        }
        if self.contains(Self::MESSAGE) {
            kinds.extend([
                // The delete events remove the message buffers of deleted channels.
                EventType::ChannelDelete,
                EventType::ThreadDelete,
                EventType::MessageCreate,
                EventType::MessageUpdate,
                EventType::MessageDelete,
                EventType::MessageDeleteBulk,
                EventType::MessageReactionAdd,
                EventType::MessageReactionRemove,
                EventType::MessageReactionRemoveAll,
                EventType::MessageReactionRemoveEmoji,
            ]);
        }
        if self.contains(Self::STAGE_INSTANCE) {
            kinds.extend([
                EventType::StageInstanceCreate,
                EventType::StageInstanceUpdate,
                EventType::StageInstanceDelete,
            ]);
        }
        if self.contains(Self::SCHEDULED_EVENT) {
            kinds.extend([
                EventType::GuildScheduledEventCreate,
                EventType::GuildScheduledEventUpdate,
                EventType::GuildScheduledEventDelete,
            ]);
        }
        if self.contains(Self::SOUNDBOARD_SOUND) {
            kinds.extend([
                EventType::GuildSoundboardSoundCreate,
                EventType::GuildSoundboardSoundUpdate,
                EventType::GuildSoundboardSoundDelete,
                EventType::GuildSoundboardSoundsUpdate,
                EventType::SoundboardSounds,
            ]);
        }
        kinds.into_iter().collect()
    }

    /// Gives the intents that the cache needs for these resources.
    #[must_use]
    pub fn intents(self) -> Intents {
        let mut intents = Intents::GUILDS;
        if self.contains(Self::MEMBER) {
            intents |= Intents::GUILD_MEMBERS;
        }
        if self.contains(Self::EMOJI)
            || self.contains(Self::STICKER)
            || self.contains(Self::SOUNDBOARD_SOUND)
        {
            intents |= Intents::GUILD_EXPRESSIONS;
        }
        if self.contains(Self::VOICE_STATE) {
            intents |= Intents::GUILD_VOICE_STATES;
        }
        if self.contains(Self::PRESENCE) {
            intents |= Intents::GUILD_PRESENCES;
        }
        if self.contains(Self::MESSAGE) {
            intents |= Intents::GUILD_MESSAGES
                | Intents::DIRECT_MESSAGES
                | Intents::GUILD_MESSAGE_REACTIONS
                | Intents::DIRECT_MESSAGE_REACTIONS;
        }
        if self.contains(Self::SCHEDULED_EVENT) {
            intents |= Intents::GUILD_SCHEDULED_EVENTS;
        }
        intents
    }
}

/// The in-memory cache. It is cheap to clone; clones share the data.
#[derive(Clone, Debug)]
pub struct Cache {
    inner: Arc<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    resources: ResourceTypes,
    message_capacity: usize,
    current_user: RwLock<Option<Arc<User>>>,
    unavailable_guilds: DashMap<GuildId, ()>,

    guilds: DashMap<GuildId, Arc<CachedGuild>>,
    channels: DashMap<ChannelId, Arc<Channel>>,
    roles: DashMap<RoleId, Arc<CachedRole>>,
    members: DashMap<(GuildId, UserId), Arc<CachedMember>>,
    users: DashMap<UserId, Arc<User>>,
    emojis: DashMap<EmojiId, (GuildId, Arc<Emoji>)>,
    stickers: DashMap<StickerId, Arc<Sticker>>,
    voice_states: DashMap<(GuildId, UserId), Arc<VoiceState>>,
    presences: DashMap<(GuildId, UserId), Arc<Presence>>,
    messages: DashMap<ChannelId, VecDeque<Arc<Message>>>,
    // The guild of each message buffer, so that a guild removal removes its buffers also
    // when the cache does not keep channels.
    message_guilds: DashMap<GuildId, HashSet<ChannelId>>,
    stage_instances: DashMap<StageInstanceId, Arc<StageInstance>>,
    scheduled_events: DashMap<ScheduledEventId, Arc<ScheduledEvent>>,
    soundboard_sounds: DashMap<SoundboardSoundId, Arc<SoundboardSound>>,

    // Indexes from a guild to its objects, so that a guild removal removes all of them.
    guild_channels: DashMap<GuildId, HashSet<ChannelId>>,
    guild_roles: DashMap<GuildId, HashSet<RoleId>>,
    guild_members: DashMap<GuildId, HashSet<UserId>>,
    guild_emojis: DashMap<GuildId, HashSet<EmojiId>>,
    guild_stickers: DashMap<GuildId, HashSet<StickerId>>,
    guild_stage_instances: DashMap<GuildId, HashSet<StageInstanceId>>,
    guild_scheduled_events: DashMap<GuildId, HashSet<ScheduledEventId>>,
    guild_soundboard_sounds: DashMap<GuildId, HashSet<SoundboardSoundId>>,
    // The guilds of each user, so that a user is removed with its last member.
    user_guilds: DashMap<UserId, HashSet<GuildId>>,
}

impl Cache {
    /// Makes a cache for resource types. The message cache keeps 100 messages for each
    /// channel; change it with [`Cache::with_message_capacity`].
    #[must_use]
    pub fn new(resources: ResourceTypes) -> Self {
        Self::with_message_capacity(resources, 100)
    }

    /// Makes a cache for resource types, with a number of messages to keep for each channel.
    #[must_use]
    pub fn with_message_capacity(resources: ResourceTypes, message_capacity: usize) -> Self {
        Self {
            inner: Arc::new(Inner {
                resources,
                message_capacity,
                ..Inner::default()
            }),
        }
    }

    /// Gives the resource types that the cache keeps.
    #[must_use]
    pub fn resources(&self) -> ResourceTypes {
        self.inner.resources
    }

    fn wants(&self, resource: ResourceTypes) -> bool {
        self.inner.resources.contains(resource)
    }

    /// Gives the current user.
    #[must_use]
    pub fn current_user(&self) -> Option<Arc<User>> {
        self.inner
            .current_user
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Gives a guild.
    #[must_use]
    pub fn guild(&self, guild_id: GuildId) -> Option<Arc<CachedGuild>> {
        get(&self.inner.guilds, &guild_id)
    }

    /// Tells if a guild is not available (an outage, or not yet received after `READY`).
    #[must_use]
    pub fn is_guild_unavailable(&self, guild_id: GuildId) -> bool {
        self.inner.unavailable_guilds.contains_key(&guild_id)
    }

    /// Gives the number of available guilds.
    #[must_use]
    pub fn guild_count(&self) -> usize {
        self.inner.guilds.len()
    }

    /// Gives the IDs of the available guilds.
    #[must_use]
    pub fn guild_ids(&self) -> Vec<GuildId> {
        self.inner.guilds.iter().map(|entry| *entry.key()).collect()
    }

    /// Gives a channel or a thread.
    #[must_use]
    pub fn channel(&self, channel_id: ChannelId) -> Option<Arc<Channel>> {
        get(&self.inner.channels, &channel_id)
    }

    /// Gives the channels and threads of a guild.
    #[must_use]
    pub fn guild_channels(&self, guild_id: GuildId) -> Vec<Arc<Channel>> {
        self.collect(&self.inner.guild_channels, guild_id, &self.inner.channels)
    }

    /// Gives a role.
    #[must_use]
    pub fn role(&self, role_id: RoleId) -> Option<Arc<CachedRole>> {
        get(&self.inner.roles, &role_id)
    }

    /// Gives the roles of a guild.
    #[must_use]
    pub fn guild_roles(&self, guild_id: GuildId) -> Vec<Arc<CachedRole>> {
        self.collect(&self.inner.guild_roles, guild_id, &self.inner.roles)
    }

    /// Gives a member.
    #[must_use]
    pub fn member(&self, guild_id: GuildId, user_id: UserId) -> Option<Arc<CachedMember>> {
        get(&self.inner.members, &(guild_id, user_id))
    }

    /// Gives the members of a guild that the cache has.
    #[must_use]
    pub fn guild_members(&self, guild_id: GuildId) -> Vec<Arc<CachedMember>> {
        let ids: Vec<UserId> = self
            .inner
            .guild_members
            .get(&guild_id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();
        ids.into_iter()
            .filter_map(|user_id| self.member(guild_id, user_id))
            .collect()
    }

    /// Gives a user.
    #[must_use]
    pub fn user(&self, user_id: UserId) -> Option<Arc<User>> {
        get(&self.inner.users, &user_id)
    }

    /// Gives an emoji and its guild.
    #[must_use]
    pub fn emoji(&self, emoji_id: EmojiId) -> Option<(GuildId, Arc<Emoji>)> {
        self.inner
            .emojis
            .get(&emoji_id)
            .map(|entry| (entry.0, Arc::clone(&entry.1)))
    }

    /// Gives the emojis of a guild.
    #[must_use]
    pub fn guild_emojis(&self, guild_id: GuildId) -> Vec<Arc<Emoji>> {
        let ids: Vec<EmojiId> = self
            .inner
            .guild_emojis
            .get(&guild_id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();
        ids.into_iter()
            .filter_map(|id| self.emoji(id).map(|(_, emoji)| emoji))
            .collect()
    }

    /// Gives a sticker.
    #[must_use]
    pub fn sticker(&self, sticker_id: StickerId) -> Option<Arc<Sticker>> {
        get(&self.inner.stickers, &sticker_id)
    }

    /// Gives the stickers of a guild.
    #[must_use]
    pub fn guild_stickers(&self, guild_id: GuildId) -> Vec<Arc<Sticker>> {
        self.collect(&self.inner.guild_stickers, guild_id, &self.inner.stickers)
    }

    /// Gives the voice state of a user in a guild.
    #[must_use]
    pub fn voice_state(&self, guild_id: GuildId, user_id: UserId) -> Option<Arc<VoiceState>> {
        get(&self.inner.voice_states, &(guild_id, user_id))
    }

    /// Gives the voice states of the users in a voice channel.
    #[must_use]
    pub fn voice_channel_states(&self, channel_id: ChannelId) -> Vec<Arc<VoiceState>> {
        self.inner
            .voice_states
            .iter()
            .filter(|entry| entry.value().channel_id == Some(channel_id))
            .map(|entry| Arc::clone(entry.value()))
            .collect()
    }

    /// Gives the presence of a user in a guild.
    #[must_use]
    pub fn presence(&self, guild_id: GuildId, user_id: UserId) -> Option<Arc<Presence>> {
        get(&self.inner.presences, &(guild_id, user_id))
    }

    /// Gives a cached message.
    #[must_use]
    pub fn message(&self, channel_id: ChannelId, message_id: MessageId) -> Option<Arc<Message>> {
        self.inner.messages.get(&channel_id).and_then(|messages| {
            messages
                .iter()
                .find(|message| message.id == message_id)
                .cloned()
        })
    }

    /// Gives the cached messages of a channel, oldest first.
    #[must_use]
    pub fn channel_messages(&self, channel_id: ChannelId) -> Vec<Arc<Message>> {
        self.inner
            .messages
            .get(&channel_id)
            .map(|messages| messages.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Gives a stage instance.
    #[must_use]
    pub fn stage_instance(&self, stage_id: StageInstanceId) -> Option<Arc<StageInstance>> {
        get(&self.inner.stage_instances, &stage_id)
    }

    /// Gives a scheduled event.
    #[must_use]
    pub fn scheduled_event(&self, event_id: ScheduledEventId) -> Option<Arc<ScheduledEvent>> {
        get(&self.inner.scheduled_events, &event_id)
    }

    /// Gives the scheduled events of a guild.
    #[must_use]
    pub fn guild_scheduled_events(&self, guild_id: GuildId) -> Vec<Arc<ScheduledEvent>> {
        self.collect(
            &self.inner.guild_scheduled_events,
            guild_id,
            &self.inner.scheduled_events,
        )
    }

    /// Gives a soundboard sound.
    #[must_use]
    pub fn soundboard_sound(&self, sound_id: SoundboardSoundId) -> Option<Arc<SoundboardSound>> {
        get(&self.inner.soundboard_sounds, &sound_id)
    }

    /// Gives the soundboard sounds of a guild.
    #[must_use]
    pub fn guild_soundboard_sounds(&self, guild_id: GuildId) -> Vec<Arc<SoundboardSound>> {
        self.collect(
            &self.inner.guild_soundboard_sounds,
            guild_id,
            &self.inner.soundboard_sounds,
        )
    }

    /// Gives the number of cached objects of each kind: guilds, channels, roles, members,
    /// users, messages. For metrics.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            guilds: self.inner.guilds.len(),
            channels: self.inner.channels.len(),
            roles: self.inner.roles.len(),
            members: self.inner.members.len(),
            users: self.inner.users.len(),
            messages: self.inner.messages.iter().map(|entry| entry.len()).sum(),
        }
    }

    fn collect<K, V>(
        &self,
        index: &DashMap<GuildId, HashSet<K>>,
        guild_id: GuildId,
        values: &DashMap<K, Arc<V>>,
    ) -> Vec<Arc<V>>
    where
        K: Copy + Eq + Hash,
    {
        let ids: Vec<K> = index
            .get(&guild_id)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();
        ids.into_iter().filter_map(|id| get(values, &id)).collect()
    }
}

/// The number of cached objects of each kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CacheStats {
    /// Guilds.
    pub guilds: usize,
    /// Channels and threads.
    pub channels: usize,
    /// Roles.
    pub roles: usize,
    /// Members.
    pub members: usize,
    /// Users.
    pub users: usize,
    /// Messages.
    pub messages: usize,
}

fn get<K: Eq + Hash, V>(map: &DashMap<K, Arc<V>>, key: &K) -> Option<Arc<V>> {
    // Clone the Arc and release the shard lock at once.
    map.get(key).map(|entry| Arc::clone(entry.value()))
}
