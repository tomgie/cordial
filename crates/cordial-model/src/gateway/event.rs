//! Gateway dispatch events.
//!
//! Each event has its own payload type, so that a handler can name the event by type.
//! [`Event`] wraps all of them. [`Event::decode`] makes an event from its name and raw data.

use std::ops::{Deref, DerefMut};

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use super::Intents;
use crate::application::PartialApplication;
use crate::audit_log::AuditLogEntry;
use crate::auto_moderation::{AutoModerationAction, AutoModerationRule, AutoModerationTriggerType};
use crate::channel::{Channel, ThreadMember, VoiceChannelInfo};
use crate::command::GuildCommandPermissions;
use crate::emoji::{Emoji, PartialEmoji};
use crate::gateway::presence::Presence;
use crate::guild::{Guild, Integration, Member, Role, UnavailableGuild};
use crate::id::{
    ApplicationId, AutoModerationRuleId, ChannelId, GenericId, GuildId, IntegrationId, MessageId,
    RoleId, ScheduledEventId, SoundboardSoundId, UserId,
};
use crate::interaction::Interaction;
use crate::invite::InviteTargetType;
use crate::message::{Message, ReactionKind};
use crate::monetization::{Entitlement, Subscription};
use crate::scheduled_event::ScheduledEvent;
use crate::soundboard::SoundboardSound;
use crate::sticker::Sticker;
use crate::user::User;
use crate::voice::{StageInstance, VoiceState};
use crate::{Timestamp, application::Application};

/// A type that is the payload of one dispatch event.
pub trait GatewayEvent: Sized {
    /// The event type.
    const TYPE: EventType;

    /// Takes the payload out of an event of this type.
    fn from_event(event: Event) -> Option<Self>;

    /// Gives the payload of an event of this type.
    fn from_event_ref(event: &Event) -> Option<&Self>;
}

/// An event that this version of the library does not know.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct UnknownEvent {
    /// The event name.
    pub name: String,
    /// The raw JSON data.
    pub data: Box<RawValue>,
}

impl PartialEq for UnknownEvent {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.data.get() == other.data.get()
    }
}

/// A set of event types.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct EventTypes {
    bits: u128,
}

impl EventTypes {
    /// Gives an empty set.
    #[must_use]
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    /// Gives the set of all known event types.
    #[must_use]
    pub fn all() -> Self {
        EventType::ALL.iter().copied().collect()
    }

    /// Tells if the set has an event type.
    #[must_use]
    pub const fn contains(self, kind: EventType) -> bool {
        self.bits & kind.bit() != 0
    }

    /// Adds an event type.
    pub const fn insert(&mut self, kind: EventType) {
        self.bits |= kind.bit();
    }

    /// Adds all event types of another set.
    pub const fn extend_from(&mut self, other: Self) {
        self.bits |= other.bits;
    }

    /// Tells if the set is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    /// Gives the event types in the set.
    pub fn iter(self) -> impl Iterator<Item = EventType> {
        EventType::ALL
            .iter()
            .copied()
            .filter(move |kind| self.contains(*kind))
    }

    /// Gives the intents that the event types in the set need.
    #[must_use]
    pub fn intents(self) -> Intents {
        self.iter().map(EventType::intents).collect()
    }
}

impl FromIterator<EventType> for EventTypes {
    fn from_iter<I: IntoIterator<Item = EventType>>(iterator: I) -> Self {
        let mut set = Self::empty();
        for kind in iterator {
            set.insert(kind);
        }
        set
    }
}

fn decode<T: for<'de> Deserialize<'de>>(data: &RawValue) -> Result<Box<T>, serde_json::Error> {
    serde_json::from_str(data.get()).map(Box::new)
}

macro_rules! events {
    ($(
        $(#[$meta:meta])*
        $variant:ident = $name:literal => $payload:ident, $intents:expr;
    )*) => {
        /// A gateway dispatch event.
        #[derive(Clone, Debug, PartialEq)]
        #[non_exhaustive]
        pub enum Event {
            $(
                $(#[$meta])*
                $variant(Box<$payload>),
            )*
            /// An event that this version of the library does not know.
            Unknown(UnknownEvent),
        }

        /// The type of a gateway dispatch event.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[non_exhaustive]
        pub enum EventType {
            $(
                $(#[$meta])*
                $variant,
            )*
        }

        impl EventType {
            /// All event types.
            pub const ALL: &'static [EventType] = &[$(EventType::$variant,)*];

            /// Gives the event name that Discord uses, for example `MESSAGE_CREATE`.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }

            /// Gives the event type for a name.
            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Self::$variant),)*
                    _ => None,
                }
            }

            /// Gives the intents that enable the event. Any one of them is enough.
            /// An empty set means that the event needs no intent.
            #[must_use]
            pub const fn intents(self) -> Intents {
                match self {
                    $(Self::$variant => $intents,)*
                }
            }

            const fn bit(self) -> u128 {
                1 << (self as u32)
            }
        }

        impl Event {
            /// Gives the event type, or `None` for an unknown event.
            #[must_use]
            pub fn kind(&self) -> Option<EventType> {
                match self {
                    $(Self::$variant(_) => Some(EventType::$variant),)*
                    Self::Unknown(_) => None,
                }
            }

            /// Gives the event name that Discord uses.
            #[must_use]
            pub fn name(&self) -> &str {
                match self {
                    $(Self::$variant(_) => $name,)*
                    Self::Unknown(unknown) => &unknown.name,
                }
            }

            /// Makes an event from its name and raw data.
            ///
            /// # Errors
            ///
            /// Gives an error when the data does not match the event type.
            pub fn decode(name: &str, data: &RawValue) -> Result<Self, serde_json::Error> {
                match name {
                    $($name => decode::<$payload>(data).map(Self::$variant),)*
                    _ => Ok(Self::Unknown(UnknownEvent {
                        name: name.to_owned(),
                        data: data.to_owned(),
                    })),
                }
            }
        }

        $(
            impl GatewayEvent for $payload {
                const TYPE: EventType = EventType::$variant;

                fn from_event(event: Event) -> Option<Self> {
                    match event {
                        Event::$variant(payload) => Some(*payload),
                        _ => None,
                    }
                }

                fn from_event_ref(event: &Event) -> Option<&Self> {
                    match event {
                        Event::$variant(payload) => Some(payload),
                        _ => None,
                    }
                }
            }
        )*
    };
}

const NONE: Intents = Intents::empty();
const GUILDS: Intents = Intents::GUILDS;
const MESSAGES: Intents = Intents::GUILD_MESSAGES.union(Intents::DIRECT_MESSAGES);
const REACTIONS: Intents =
    Intents::GUILD_MESSAGE_REACTIONS.union(Intents::DIRECT_MESSAGE_REACTIONS);
const POLLS: Intents = Intents::GUILD_MESSAGE_POLLS.union(Intents::DIRECT_MESSAGE_POLLS);
const TYPING: Intents = Intents::GUILD_MESSAGE_TYPING.union(Intents::DIRECT_MESSAGE_TYPING);

events! {
    /// The session is ready.
    Ready = "READY" => Ready, NONE;
    /// The session resumed.
    Resumed = "RESUMED" => Resumed, NONE;
    /// A gateway command was rate limited.
    RateLimited = "RATE_LIMITED" => RateLimited, NONE;
    /// The permissions of a command changed.
    ApplicationCommandPermissionsUpdate = "APPLICATION_COMMAND_PERMISSIONS_UPDATE" => ApplicationCommandPermissionsUpdate, NONE;
    /// An auto moderation rule was made.
    AutoModerationRuleCreate = "AUTO_MODERATION_RULE_CREATE" => AutoModerationRuleCreate, Intents::AUTO_MODERATION_CONFIGURATION;
    /// An auto moderation rule changed.
    AutoModerationRuleUpdate = "AUTO_MODERATION_RULE_UPDATE" => AutoModerationRuleUpdate, Intents::AUTO_MODERATION_CONFIGURATION;
    /// An auto moderation rule was deleted.
    AutoModerationRuleDelete = "AUTO_MODERATION_RULE_DELETE" => AutoModerationRuleDelete, Intents::AUTO_MODERATION_CONFIGURATION;
    /// An auto moderation rule triggered and an action ran.
    AutoModerationActionExecution = "AUTO_MODERATION_ACTION_EXECUTION" => AutoModerationActionExecution, Intents::AUTO_MODERATION_EXECUTION;
    /// A channel was made.
    ChannelCreate = "CHANNEL_CREATE" => ChannelCreate, GUILDS;
    /// A channel changed.
    ChannelUpdate = "CHANNEL_UPDATE" => ChannelUpdate, GUILDS;
    /// A channel was deleted.
    ChannelDelete = "CHANNEL_DELETE" => ChannelDelete, GUILDS;
    /// The reply to Request Channel Info.
    ChannelInfo = "CHANNEL_INFO" => ChannelInfo, NONE;
    /// A message was pinned or unpinned.
    ChannelPinsUpdate = "CHANNEL_PINS_UPDATE" => ChannelPinsUpdate, GUILDS.union(Intents::DIRECT_MESSAGES);
    /// A thread was made, or the current user was added to a private thread.
    ThreadCreate = "THREAD_CREATE" => ThreadCreate, GUILDS;
    /// A thread changed.
    ThreadUpdate = "THREAD_UPDATE" => ThreadUpdate, GUILDS;
    /// A thread was deleted.
    ThreadDelete = "THREAD_DELETE" => ThreadDelete, GUILDS;
    /// The active threads of channels that the current user can now access.
    ThreadListSync = "THREAD_LIST_SYNC" => ThreadListSync, GUILDS;
    /// The thread member of the current user changed.
    ThreadMemberUpdate = "THREAD_MEMBER_UPDATE" => ThreadMemberUpdate, GUILDS;
    /// Users were added to or removed from a thread.
    ThreadMembersUpdate = "THREAD_MEMBERS_UPDATE" => ThreadMembersUpdate, GUILDS.union(Intents::GUILD_MEMBERS);
    /// An entitlement was made.
    EntitlementCreate = "ENTITLEMENT_CREATE" => EntitlementCreate, NONE;
    /// An entitlement changed.
    EntitlementUpdate = "ENTITLEMENT_UPDATE" => EntitlementUpdate, NONE;
    /// An entitlement was deleted.
    EntitlementDelete = "ENTITLEMENT_DELETE" => EntitlementDelete, NONE;
    /// A guild became available, or the current user joined a guild.
    GuildCreate = "GUILD_CREATE" => GuildCreate, GUILDS;
    /// A guild changed.
    GuildUpdate = "GUILD_UPDATE" => GuildUpdate, GUILDS;
    /// A guild became unavailable, or the current user left it or was removed.
    GuildDelete = "GUILD_DELETE" => GuildDelete, GUILDS;
    /// An audit log entry was made.
    GuildAuditLogEntryCreate = "GUILD_AUDIT_LOG_ENTRY_CREATE" => GuildAuditLogEntryCreate, Intents::GUILD_MODERATION;
    /// A user was banned.
    GuildBanAdd = "GUILD_BAN_ADD" => GuildBanAdd, Intents::GUILD_MODERATION;
    /// A ban was removed.
    GuildBanRemove = "GUILD_BAN_REMOVE" => GuildBanRemove, Intents::GUILD_MODERATION;
    /// The emojis of a guild changed.
    GuildEmojisUpdate = "GUILD_EMOJIS_UPDATE" => GuildEmojisUpdate, Intents::GUILD_EXPRESSIONS;
    /// The stickers of a guild changed.
    GuildStickersUpdate = "GUILD_STICKERS_UPDATE" => GuildStickersUpdate, Intents::GUILD_EXPRESSIONS;
    /// The integrations of a guild changed.
    GuildIntegrationsUpdate = "GUILD_INTEGRATIONS_UPDATE" => GuildIntegrationsUpdate, Intents::GUILD_INTEGRATIONS;
    /// A user joined a guild.
    GuildMemberAdd = "GUILD_MEMBER_ADD" => GuildMemberAdd, Intents::GUILD_MEMBERS;
    /// A user left a guild or was removed.
    GuildMemberRemove = "GUILD_MEMBER_REMOVE" => GuildMemberRemove, Intents::GUILD_MEMBERS;
    /// A member changed.
    GuildMemberUpdate = "GUILD_MEMBER_UPDATE" => GuildMemberUpdate, Intents::GUILD_MEMBERS;
    /// The reply to Request Guild Members.
    GuildMembersChunk = "GUILD_MEMBERS_CHUNK" => GuildMembersChunk, NONE;
    /// A role was made.
    GuildRoleCreate = "GUILD_ROLE_CREATE" => GuildRoleCreate, GUILDS;
    /// A role changed.
    GuildRoleUpdate = "GUILD_ROLE_UPDATE" => GuildRoleUpdate, GUILDS;
    /// A role was deleted.
    GuildRoleDelete = "GUILD_ROLE_DELETE" => GuildRoleDelete, GUILDS;
    /// A scheduled event was made.
    GuildScheduledEventCreate = "GUILD_SCHEDULED_EVENT_CREATE" => GuildScheduledEventCreate, Intents::GUILD_SCHEDULED_EVENTS;
    /// A scheduled event changed.
    GuildScheduledEventUpdate = "GUILD_SCHEDULED_EVENT_UPDATE" => GuildScheduledEventUpdate, Intents::GUILD_SCHEDULED_EVENTS;
    /// A scheduled event was deleted.
    GuildScheduledEventDelete = "GUILD_SCHEDULED_EVENT_DELETE" => GuildScheduledEventDelete, Intents::GUILD_SCHEDULED_EVENTS;
    /// A user subscribed to a scheduled event.
    GuildScheduledEventUserAdd = "GUILD_SCHEDULED_EVENT_USER_ADD" => GuildScheduledEventUserAdd, Intents::GUILD_SCHEDULED_EVENTS;
    /// A user unsubscribed from a scheduled event.
    GuildScheduledEventUserRemove = "GUILD_SCHEDULED_EVENT_USER_REMOVE" => GuildScheduledEventUserRemove, Intents::GUILD_SCHEDULED_EVENTS;
    /// A soundboard sound was made.
    GuildSoundboardSoundCreate = "GUILD_SOUNDBOARD_SOUND_CREATE" => GuildSoundboardSoundCreate, Intents::GUILD_EXPRESSIONS;
    /// A soundboard sound changed.
    GuildSoundboardSoundUpdate = "GUILD_SOUNDBOARD_SOUND_UPDATE" => GuildSoundboardSoundUpdate, Intents::GUILD_EXPRESSIONS;
    /// A soundboard sound was deleted.
    GuildSoundboardSoundDelete = "GUILD_SOUNDBOARD_SOUND_DELETE" => GuildSoundboardSoundDelete, Intents::GUILD_EXPRESSIONS;
    /// The soundboard sounds of a guild changed.
    GuildSoundboardSoundsUpdate = "GUILD_SOUNDBOARD_SOUNDS_UPDATE" => GuildSoundboardSoundsUpdate, Intents::GUILD_EXPRESSIONS;
    /// The reply to Request Soundboard Sounds.
    SoundboardSounds = "SOUNDBOARD_SOUNDS" => SoundboardSounds, NONE;
    /// An integration was made.
    IntegrationCreate = "INTEGRATION_CREATE" => IntegrationCreate, Intents::GUILD_INTEGRATIONS;
    /// An integration changed.
    IntegrationUpdate = "INTEGRATION_UPDATE" => IntegrationUpdate, Intents::GUILD_INTEGRATIONS;
    /// An integration was deleted.
    IntegrationDelete = "INTEGRATION_DELETE" => IntegrationDelete, Intents::GUILD_INTEGRATIONS;
    /// A user used a command, a component, or a modal.
    InteractionCreate = "INTERACTION_CREATE" => InteractionCreate, NONE;
    /// An invite was made.
    InviteCreate = "INVITE_CREATE" => InviteCreate, Intents::GUILD_INVITES;
    /// An invite was deleted.
    InviteDelete = "INVITE_DELETE" => InviteDelete, Intents::GUILD_INVITES;
    /// A message was sent.
    MessageCreate = "MESSAGE_CREATE" => MessageCreate, MESSAGES;
    /// A message was edited.
    MessageUpdate = "MESSAGE_UPDATE" => MessageUpdate, MESSAGES;
    /// A message was deleted.
    MessageDelete = "MESSAGE_DELETE" => MessageDelete, MESSAGES;
    /// Messages were deleted in bulk.
    MessageDeleteBulk = "MESSAGE_DELETE_BULK" => MessageDeleteBulk, Intents::GUILD_MESSAGES;
    /// A user reacted to a message.
    MessageReactionAdd = "MESSAGE_REACTION_ADD" => MessageReactionAdd, REACTIONS;
    /// A user removed a reaction.
    MessageReactionRemove = "MESSAGE_REACTION_REMOVE" => MessageReactionRemove, REACTIONS;
    /// All reactions of a message were removed.
    MessageReactionRemoveAll = "MESSAGE_REACTION_REMOVE_ALL" => MessageReactionRemoveAll, REACTIONS;
    /// All reactions with one emoji were removed.
    MessageReactionRemoveEmoji = "MESSAGE_REACTION_REMOVE_EMOJI" => MessageReactionRemoveEmoji, REACTIONS;
    /// A user voted in a poll.
    MessagePollVoteAdd = "MESSAGE_POLL_VOTE_ADD" => MessagePollVoteAdd, POLLS;
    /// A user removed a vote in a poll.
    MessagePollVoteRemove = "MESSAGE_POLL_VOTE_REMOVE" => MessagePollVoteRemove, POLLS;
    /// The presence of a user changed.
    PresenceUpdate = "PRESENCE_UPDATE" => PresenceUpdate, Intents::GUILD_PRESENCES;
    /// A stage instance was made.
    StageInstanceCreate = "STAGE_INSTANCE_CREATE" => StageInstanceCreate, GUILDS;
    /// A stage instance changed.
    StageInstanceUpdate = "STAGE_INSTANCE_UPDATE" => StageInstanceUpdate, GUILDS;
    /// A stage instance was deleted.
    StageInstanceDelete = "STAGE_INSTANCE_DELETE" => StageInstanceDelete, GUILDS;
    /// A subscription was made.
    SubscriptionCreate = "SUBSCRIPTION_CREATE" => SubscriptionCreate, NONE;
    /// A subscription changed.
    SubscriptionUpdate = "SUBSCRIPTION_UPDATE" => SubscriptionUpdate, NONE;
    /// A subscription was deleted.
    SubscriptionDelete = "SUBSCRIPTION_DELETE" => SubscriptionDelete, NONE;
    /// A user started typing.
    TypingStart = "TYPING_START" => TypingStart, TYPING;
    /// The current user changed.
    UserUpdate = "USER_UPDATE" => UserUpdate, NONE;
    /// A user sent an effect in a voice channel.
    VoiceChannelEffectSend = "VOICE_CHANNEL_EFFECT_SEND" => VoiceChannelEffectSend, Intents::GUILD_VOICE_STATES;
    /// The start time of a voice channel session changed.
    VoiceChannelStartTimeUpdate = "VOICE_CHANNEL_START_TIME_UPDATE" => VoiceChannelStartTimeUpdate, GUILDS;
    /// The status of a voice channel changed.
    VoiceChannelStatusUpdate = "VOICE_CHANNEL_STATUS_UPDATE" => VoiceChannelStatusUpdate, GUILDS;
    /// A user joined, left, or moved between voice channels.
    VoiceStateUpdate = "VOICE_STATE_UPDATE" => VoiceStateUpdate, Intents::GUILD_VOICE_STATES;
    /// The voice server of a guild changed.
    VoiceServerUpdate = "VOICE_SERVER_UPDATE" => VoiceServerUpdate, NONE;
    /// A webhook of a channel was made, changed, or deleted.
    WebhooksUpdate = "WEBHOOKS_UPDATE" => WebhooksUpdate, Intents::GUILD_WEBHOOKS;
}

/// Makes event types that wrap an existing object.
macro_rules! wrapper_events {
    ($($(#[$meta:meta])* $name:ident($inner:ty);)*) => {
        $(
            $(#[$meta])*
            #[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
            #[serde(transparent)]
            pub struct $name(pub $inner);

            impl Deref for $name {
                type Target = $inner;

                fn deref(&self) -> &$inner {
                    &self.0
                }
            }

            impl DerefMut for $name {
                fn deref_mut(&mut self) -> &mut $inner {
                    &mut self.0
                }
            }
        )*
    };
}

wrapper_events! {
    /// The payload of `AUTO_MODERATION_RULE_CREATE`.
    AutoModerationRuleCreate(AutoModerationRule);
    /// The payload of `AUTO_MODERATION_RULE_UPDATE`.
    AutoModerationRuleUpdate(AutoModerationRule);
    /// The payload of `AUTO_MODERATION_RULE_DELETE`.
    AutoModerationRuleDelete(AutoModerationRule);
    /// The payload of `APPLICATION_COMMAND_PERMISSIONS_UPDATE`.
    ApplicationCommandPermissionsUpdate(GuildCommandPermissions);
    /// The payload of `CHANNEL_CREATE`.
    ChannelCreate(Channel);
    /// The payload of `CHANNEL_UPDATE`.
    ChannelUpdate(Channel);
    /// The payload of `CHANNEL_DELETE`.
    ChannelDelete(Channel);
    /// The payload of `THREAD_CREATE`.
    ThreadCreate(Channel);
    /// The payload of `THREAD_UPDATE`.
    ThreadUpdate(Channel);
    /// The payload of `THREAD_DELETE`. Only `id`, `guild_id`, `parent_id`, and `kind` are set.
    ThreadDelete(Channel);
    /// The payload of `THREAD_MEMBER_UPDATE`. It has `guild_id`.
    ThreadMemberUpdate(ThreadMember);
    /// The payload of `ENTITLEMENT_CREATE`.
    EntitlementCreate(Entitlement);
    /// The payload of `ENTITLEMENT_UPDATE`.
    EntitlementUpdate(Entitlement);
    /// The payload of `ENTITLEMENT_DELETE`.
    EntitlementDelete(Entitlement);
    /// The payload of `GUILD_UPDATE`.
    GuildUpdate(Guild);
    /// The payload of `GUILD_DELETE`. `unavailable` tells an outage from a removal.
    GuildDelete(UnavailableGuild);
    /// The payload of `GUILD_AUDIT_LOG_ENTRY_CREATE`. It has `guild_id`.
    GuildAuditLogEntryCreate(AuditLogEntry);
    /// The payload of `GUILD_MEMBER_ADD`. It has `guild_id`.
    GuildMemberAdd(Member);
    /// The payload of `GUILD_SCHEDULED_EVENT_CREATE`.
    GuildScheduledEventCreate(ScheduledEvent);
    /// The payload of `GUILD_SCHEDULED_EVENT_UPDATE`.
    GuildScheduledEventUpdate(ScheduledEvent);
    /// The payload of `GUILD_SCHEDULED_EVENT_DELETE`.
    GuildScheduledEventDelete(ScheduledEvent);
    /// The payload of `GUILD_SOUNDBOARD_SOUND_CREATE`.
    GuildSoundboardSoundCreate(SoundboardSound);
    /// The payload of `GUILD_SOUNDBOARD_SOUND_UPDATE`.
    GuildSoundboardSoundUpdate(SoundboardSound);
    /// The payload of `INTEGRATION_CREATE`. It has `guild_id`.
    IntegrationCreate(Integration);
    /// The payload of `INTEGRATION_UPDATE`. It has `guild_id`.
    IntegrationUpdate(Integration);
    /// The payload of `INTERACTION_CREATE`.
    InteractionCreate(Interaction);
    /// The payload of `MESSAGE_CREATE`.
    MessageCreate(Message);
    /// The payload of `MESSAGE_UPDATE`.
    MessageUpdate(Message);
    /// The payload of `PRESENCE_UPDATE`.
    PresenceUpdate(Presence);
    /// The payload of `STAGE_INSTANCE_CREATE`.
    StageInstanceCreate(StageInstance);
    /// The payload of `STAGE_INSTANCE_UPDATE`.
    StageInstanceUpdate(StageInstance);
    /// The payload of `STAGE_INSTANCE_DELETE`.
    StageInstanceDelete(StageInstance);
    /// The payload of `SUBSCRIPTION_CREATE`.
    SubscriptionCreate(Subscription);
    /// The payload of `SUBSCRIPTION_UPDATE`.
    SubscriptionUpdate(Subscription);
    /// The payload of `SUBSCRIPTION_DELETE`.
    SubscriptionDelete(Subscription);
    /// The payload of `USER_UPDATE`.
    UserUpdate(User);
    /// The payload of `VOICE_STATE_UPDATE`.
    VoiceStateUpdate(VoiceState);
}

/// The payload of `READY`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Ready {
    /// The API version.
    pub v: u8,
    /// The current user.
    pub user: User,
    /// The guilds of this shard. They become available with `GUILD_CREATE`.
    pub guilds: Vec<UnavailableGuild>,
    /// The session ID, for resume.
    pub session_id: String,
    /// The URL to resume the session.
    pub resume_gateway_url: String,
    /// The shard ID and the shard count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shard: Option<[u32; 2]>,
    /// The partial application.
    pub application: PartialApplication,
}

/// The payload of `RESUMED`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Resumed {}

/// The payload of `RATE_LIMITED`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct RateLimited {
    /// The opcode that was rate limited.
    pub opcode: super::OpCode,
    /// The number of seconds to wait.
    pub retry_after: f64,
    /// More data for the opcode. For opcode 8: `guild_id` and `nonce`.
    #[serde(default)]
    pub meta: serde_json::Value,
}

impl RateLimited {
    /// For opcode 8: gives the guild ID of the request.
    #[must_use]
    pub fn guild_id(&self) -> Option<GuildId> {
        serde_json::from_value(self.meta.get("guild_id")?.clone()).ok()
    }

    /// For opcode 8: gives the nonce of the request.
    #[must_use]
    pub fn nonce(&self) -> Option<&str> {
        self.meta.get("nonce")?.as_str()
    }
}

/// The payload of `AUTO_MODERATION_ACTION_EXECUTION`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct AutoModerationActionExecution {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The action that ran.
    pub action: AutoModerationAction,
    /// The rule.
    pub rule_id: AutoModerationRuleId,
    /// The trigger type of the rule.
    pub rule_trigger_type: AutoModerationTriggerType,
    /// The user whose content triggered the rule.
    pub user_id: UserId,
    /// The channel of the content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_id: Option<ChannelId>,
    /// The message of the content. Absent when the message was blocked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<MessageId>,
    /// The alert message that the action posted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alert_system_message_id: Option<MessageId>,
    /// The content. Empty without the `MESSAGE_CONTENT` intent.
    #[serde(default)]
    pub content: String,
    /// The keyword of the rule that matched.
    #[serde(default)]
    pub matched_keyword: Option<String>,
    /// The part of the content that matched. Empty without the `MESSAGE_CONTENT` intent.
    #[serde(default)]
    pub matched_content: Option<String>,
}

/// The payload of `CHANNEL_INFO`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ChannelInfo {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The channel data.
    pub channels: Vec<VoiceChannelInfo>,
}

/// The payload of `CHANNEL_PINS_UPDATE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ChannelPinsUpdate {
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The channel ID.
    pub channel_id: ChannelId,
    /// When the last pin was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_pin_timestamp: Option<Timestamp>,
}

/// The payload of `THREAD_LIST_SYNC`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ThreadListSync {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The parent channels whose threads synced. Absent means the whole guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_ids: Option<Vec<ChannelId>>,
    /// The active threads that the current user can access.
    pub threads: Vec<Channel>,
    /// The thread members of the current user.
    pub members: Vec<ThreadMember>,
}

/// The payload of `THREAD_MEMBERS_UPDATE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct ThreadMembersUpdate {
    /// The thread ID.
    pub id: ChannelId,
    /// The guild ID.
    pub guild_id: GuildId,
    /// The approximate number of members, up to 50.
    pub member_count: u32,
    /// The users that were added.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub added_members: Vec<ThreadMember>,
    /// The users that were removed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed_member_ids: Vec<UserId>,
}

/// The payload of `GUILD_CREATE`: a guild, or an unavailable guild during an outage.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
// `Event` already boxes this payload, so the size of the large variant does not matter.
#[allow(clippy::large_enum_variant)]
pub enum GuildCreate {
    /// The guild is available.
    Available(Guild),
    /// The guild is not available because of an outage.
    Unavailable(UnavailableGuild),
}

impl GuildCreate {
    /// Gives the guild ID.
    #[must_use]
    pub fn id(&self) -> GuildId {
        match self {
            Self::Available(guild) => guild.id,
            Self::Unavailable(guild) => guild.id,
        }
    }
}

impl<'de> Deserialize<'de> for GuildCreate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Probe {
            #[serde(default)]
            unavailable: bool,
            #[serde(default)]
            name: Option<de::IgnoredAny>,
        }

        // Read the raw text once, then decode it into the right shape. This avoids a copy
        // of the whole (sometimes very large) payload into a generic value.
        let raw = <Box<RawValue>>::deserialize(deserializer)?;
        let probe: Probe = serde_json::from_str(raw.get()).map_err(de::Error::custom)?;

        if probe.unavailable && probe.name.is_none() {
            serde_json::from_str(raw.get())
                .map(Self::Unavailable)
                .map_err(de::Error::custom)
        } else {
            serde_json::from_str(raw.get())
                .map(Self::Available)
                .map_err(de::Error::custom)
        }
    }
}

/// The payload of `GUILD_BAN_ADD`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildBanAdd {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The banned user.
    pub user: User,
}

/// The payload of `GUILD_BAN_REMOVE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildBanRemove {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The user.
    pub user: User,
}

/// The payload of `GUILD_EMOJIS_UPDATE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildEmojisUpdate {
    /// The guild ID.
    pub guild_id: GuildId,
    /// All emojis of the guild.
    pub emojis: Vec<Emoji>,
}

/// The payload of `GUILD_STICKERS_UPDATE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildStickersUpdate {
    /// The guild ID.
    pub guild_id: GuildId,
    /// All stickers of the guild.
    pub stickers: Vec<Sticker>,
}

/// The payload of `GUILD_INTEGRATIONS_UPDATE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildIntegrationsUpdate {
    /// The guild ID.
    pub guild_id: GuildId,
}

/// The payload of `GUILD_MEMBER_REMOVE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildMemberRemove {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The user.
    pub user: User,
}

/// The payload of `GUILD_MEMBER_UPDATE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildMemberUpdate {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The role IDs.
    pub roles: Vec<RoleId>,
    /// The user.
    pub user: User,
    /// The nickname.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nick: Option<String>,
    /// The avatar hash in the guild.
    #[serde(default)]
    pub avatar: Option<crate::ImageHash>,
    /// The banner hash in the guild.
    #[serde(default)]
    pub banner: Option<crate::ImageHash>,
    /// When the user joined.
    #[serde(default)]
    pub joined_at: Option<Timestamp>,
    /// When the user started to boost the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub premium_since: Option<Timestamp>,
    /// Tells if the user is deafened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deaf: Option<bool>,
    /// Tells if the user is muted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mute: Option<bool>,
    /// Tells if the user has not passed Membership Screening yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<bool>,
    /// When the timeout ends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub communication_disabled_until: Option<Timestamp>,
    /// The member flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flags: Option<crate::guild::MemberFlags>,
    /// The avatar decoration in the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_decoration_data: Option<crate::user::AvatarDecorationData>,
    /// The collectibles in the guild.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collectibles: Option<crate::user::Collectibles>,
}

/// The payload of `GUILD_MEMBERS_CHUNK`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildMembersChunk {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The members.
    pub members: Vec<Member>,
    /// The index of this chunk, from 0.
    pub chunk_index: u32,
    /// The number of chunks.
    pub chunk_count: u32,
    /// The requested IDs that are not members. Discord can send them as numbers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_found: Vec<UserId>,
    /// The presences of the members, when the request asked for them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presences: Option<Vec<Presence>>,
    /// The nonce of the request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

/// The payload of `GUILD_ROLE_CREATE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildRoleCreate {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The role.
    pub role: Role,
}

/// The payload of `GUILD_ROLE_UPDATE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildRoleUpdate {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The role.
    pub role: Role,
}

/// The payload of `GUILD_ROLE_DELETE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildRoleDelete {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The role ID.
    pub role_id: RoleId,
}

/// The payload of `GUILD_SCHEDULED_EVENT_USER_ADD`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildScheduledEventUserAdd {
    /// The scheduled event.
    pub guild_scheduled_event_id: ScheduledEventId,
    /// The user.
    pub user_id: UserId,
    /// The guild ID.
    pub guild_id: GuildId,
}

/// The payload of `GUILD_SCHEDULED_EVENT_USER_REMOVE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildScheduledEventUserRemove {
    /// The scheduled event.
    pub guild_scheduled_event_id: ScheduledEventId,
    /// The user.
    pub user_id: UserId,
    /// The guild ID.
    pub guild_id: GuildId,
}

/// The payload of `GUILD_SOUNDBOARD_SOUND_DELETE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildSoundboardSoundDelete {
    /// The sound ID.
    pub sound_id: SoundboardSoundId,
    /// The guild ID.
    pub guild_id: GuildId,
}

/// The payload of `GUILD_SOUNDBOARD_SOUNDS_UPDATE`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GuildSoundboardSoundsUpdate {
    /// The sounds.
    pub soundboard_sounds: Vec<SoundboardSound>,
    /// The guild ID.
    pub guild_id: GuildId,
}

/// The payload of `SOUNDBOARD_SOUNDS`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct SoundboardSounds {
    /// The sounds.
    pub soundboard_sounds: Vec<SoundboardSound>,
    /// The guild ID.
    pub guild_id: GuildId,
}

/// The payload of `INTEGRATION_DELETE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct IntegrationDelete {
    /// The integration ID.
    pub id: IntegrationId,
    /// The guild ID.
    pub guild_id: GuildId,
    /// The application of a Discord integration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub application_id: Option<ApplicationId>,
}

/// The payload of `INVITE_CREATE`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct InviteCreate {
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The invite code.
    pub code: String,
    /// When the invite was made.
    pub created_at: Timestamp,
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The user that made the invite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inviter: Option<User>,
    /// The time in seconds after which the invite expires.
    pub max_age: u32,
    /// The maximum number of uses.
    pub max_uses: u32,
    /// The target type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_type: Option<InviteTargetType>,
    /// The user whose stream the invite shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_user: Option<User>,
    /// The embedded application of the invite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_application: Option<Box<Application>>,
    /// Tells if the invite gives temporary membership.
    pub temporary: bool,
    /// The number of uses (always 0).
    #[serde(default)]
    pub uses: u32,
    /// When the invite expires.
    #[serde(default)]
    pub expires_at: Option<Timestamp>,
    /// The roles that the invite gives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role_ids: Option<Vec<RoleId>>,
}

/// The payload of `INVITE_DELETE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct InviteDelete {
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The invite code.
    pub code: String,
}

/// The payload of `MESSAGE_DELETE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessageDelete {
    /// The message ID.
    pub id: MessageId,
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
}

/// The payload of `MESSAGE_DELETE_BULK`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessageDeleteBulk {
    /// The message IDs.
    pub ids: Vec<MessageId>,
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
}

/// The payload of `MESSAGE_REACTION_ADD`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessageReactionAdd {
    /// The user that reacted.
    pub user_id: UserId,
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The message ID.
    pub message_id: MessageId,
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The member that reacted, in guilds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
    /// The emoji.
    pub emoji: PartialEmoji,
    /// The author of the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_author_id: Option<UserId>,
    /// Tells if this is a super reaction.
    #[serde(default)]
    pub burst: bool,
    /// The colors of the super reaction, as `#rrggbb`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub burst_colors: Vec<String>,
    /// The reaction type.
    #[serde(rename = "type", default)]
    pub kind: ReactionKind,
}

/// The payload of `MESSAGE_REACTION_REMOVE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessageReactionRemove {
    /// The user whose reaction was removed.
    pub user_id: UserId,
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The message ID.
    pub message_id: MessageId,
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The emoji.
    pub emoji: PartialEmoji,
    /// Tells if this was a super reaction.
    #[serde(default)]
    pub burst: bool,
    /// The reaction type.
    #[serde(rename = "type", default)]
    pub kind: ReactionKind,
}

/// The payload of `MESSAGE_REACTION_REMOVE_ALL`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessageReactionRemoveAll {
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The message ID.
    pub message_id: MessageId,
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
}

/// The payload of `MESSAGE_REACTION_REMOVE_EMOJI`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessageReactionRemoveEmoji {
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The message ID.
    pub message_id: MessageId,
    /// The emoji.
    pub emoji: PartialEmoji,
}

/// The payload of `MESSAGE_POLL_VOTE_ADD`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessagePollVoteAdd {
    /// The user that voted.
    pub user_id: UserId,
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The message ID.
    pub message_id: MessageId,
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The answer ID.
    pub answer_id: u32,
}

/// The payload of `MESSAGE_POLL_VOTE_REMOVE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MessagePollVoteRemove {
    /// The user that removed the vote.
    pub user_id: UserId,
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The message ID.
    pub message_id: MessageId,
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The answer ID.
    pub answer_id: u32,
}

/// The payload of `TYPING_START`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct TypingStart {
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The guild ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<GuildId>,
    /// The user ID.
    pub user_id: UserId,
    /// When the user started typing.
    #[serde(with = "crate::time::unix_seconds")]
    pub timestamp: Timestamp,
    /// The member, in guilds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member: Option<Member>,
}

/// The payload of `VOICE_CHANNEL_EFFECT_SEND`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct VoiceChannelEffectSend {
    /// The channel ID.
    pub channel_id: ChannelId,
    /// The guild ID.
    pub guild_id: GuildId,
    /// The user that sent the effect.
    pub user_id: UserId,
    /// The emoji, for emoji reaction and soundboard effects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emoji: Option<PartialEmoji>,
    /// The animation type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation_type: Option<VoiceChannelEffectAnimationType>,
    /// The animation ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation_id: Option<u64>,
    /// The soundboard sound. A snowflake, or an integer for a default sound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_id: Option<GenericId>,
    /// The volume of the sound, from 0 to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound_volume: Option<f64>,
}

crate::discord_enum! {
    /// The animation type of a voice channel effect.
    pub enum VoiceChannelEffectAnimationType: u8 {
        /// A special animation from a Nitro subscriber.
        Premium = 0,
        /// The standard animation.
        Basic = 1,
    }
}

/// The payload of `VOICE_CHANNEL_START_TIME_UPDATE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct VoiceChannelStartTimeUpdate {
    /// The channel ID.
    pub id: ChannelId,
    /// The guild ID.
    pub guild_id: GuildId,
    /// When the voice session started.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "crate::time::option_unix_seconds"
    )]
    pub voice_start_time: Option<Timestamp>,
}

/// The payload of `VOICE_CHANNEL_STATUS_UPDATE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct VoiceChannelStatusUpdate {
    /// The channel ID.
    pub id: ChannelId,
    /// The guild ID.
    pub guild_id: GuildId,
    /// The new status.
    #[serde(default)]
    pub status: Option<String>,
}

/// The payload of `VOICE_SERVER_UPDATE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct VoiceServerUpdate {
    /// The voice connection token.
    pub token: crate::Secret,
    /// The guild ID.
    pub guild_id: GuildId,
    /// The voice server host. `None` means that the server is being reallocated: disconnect
    /// and wait for the next update.
    #[serde(default)]
    pub endpoint: Option<String>,
}

/// The payload of `WEBHOOKS_UPDATE`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[non_exhaustive]
pub struct WebhooksUpdate {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The channel ID.
    pub channel_id: ChannelId,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(text: &str) -> Box<RawValue> {
        RawValue::from_string(text.to_owned()).unwrap()
    }

    #[test]
    fn decodes_known_and_unknown_events() {
        let delete =
            Event::decode("MESSAGE_DELETE", &raw(r#"{"id":"2","channel_id":"3"}"#)).unwrap();
        let unknown = Event::decode("SOMETHING_NEW", &raw(r#"{"a":1}"#)).unwrap();

        assert_eq!(delete.kind(), Some(EventType::MessageDelete));
        assert!(matches!(unknown, Event::Unknown(ref event) if event.name == "SOMETHING_NEW"));
        assert_eq!(unknown.name(), "SOMETHING_NEW");
    }

    #[test]
    fn guild_create_detects_outage() {
        let unavailable =
            Event::decode("GUILD_CREATE", &raw(r#"{"id":"5","unavailable":true}"#)).unwrap();

        let Event::GuildCreate(payload) = unavailable else {
            panic!("wrong event");
        };
        assert!(matches!(*payload, GuildCreate::Unavailable(_)));
    }

    #[test]
    fn event_types_have_unique_names_and_fit_the_set() {
        assert!(EventType::ALL.len() <= 128);
        for kind in EventType::ALL {
            assert_eq!(EventType::from_name(kind.name()), Some(*kind));
        }
    }

    #[test]
    fn event_set_computes_intents() {
        let set: EventTypes = [EventType::MessageCreate, EventType::GuildMemberAdd]
            .into_iter()
            .collect();

        let intents = set.intents();

        assert!(intents.contains(Intents::GUILD_MESSAGES | Intents::GUILD_MEMBERS));
        assert!(set.contains(EventType::MessageCreate));
        assert!(!set.contains(EventType::TypingStart));
    }

    #[test]
    fn typed_access() {
        let event = Event::decode(
            "WEBHOOKS_UPDATE",
            &raw(r#"{"guild_id":"1","channel_id":"2"}"#),
        )
        .unwrap();

        let payload = WebhooksUpdate::from_event_ref(&event).unwrap();

        assert_eq!(payload.channel_id.get(), 2);
        assert!(MessageDelete::from_event_ref(&event).is_none());
    }
}
