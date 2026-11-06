//! Gateway opcodes, close codes, intents, payloads, and events.

pub mod command;
pub mod event;
pub mod presence;

pub use command::{
    Capabilities, Identify, IdentifyProperties, RequestChannelInfo, RequestGuildMembers,
    RequestSoundboardSounds, Resume, SendPayload, UpdatePresence, UpdateVoiceState,
};
pub use event::{Event, EventType, EventTypes, GatewayEvent, UnknownEvent};

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

crate::discord_enum! {
    /// A gateway opcode.
    pub enum OpCode: u8 {
        /// An event (receive).
        Dispatch = 0,
        /// A heartbeat (send and receive).
        Heartbeat = 1,
        /// Start a session (send).
        Identify = 2,
        /// Update the presence (send).
        PresenceUpdate = 3,
        /// Join, move, or leave a voice channel (send).
        VoiceStateUpdate = 4,
        /// Continue a session (send).
        Resume = 6,
        /// Reconnect and resume (receive).
        Reconnect = 7,
        /// Request guild members (send).
        RequestGuildMembers = 8,
        /// The session is not valid (receive).
        InvalidSession = 9,
        /// The first message, with the heartbeat interval (receive).
        Hello = 10,
        /// A heartbeat acknowledgement (receive).
        HeartbeatAck = 11,
        /// Request soundboard sounds (send).
        RequestSoundboardSounds = 31,
        /// Request channel info (send).
        RequestChannelInfo = 43,
    }
}

/// A gateway payload as received, with the data still as raw JSON.
///
/// Decode `d` only when a consumer wants the event. This saves CPU for unused events.
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct RawPayload<'a> {
    /// The opcode.
    pub op: OpCode,
    /// The data.
    #[serde(borrow, default)]
    pub d: Option<&'a RawValue>,
    /// The sequence number. Only for dispatch events.
    #[serde(default)]
    pub s: Option<u64>,
    /// The event name. Only for dispatch events.
    #[serde(default)]
    pub t: Option<&'a str>,
}

/// The data of the Hello payload (opcode 10).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Hello {
    /// The heartbeat interval in milliseconds.
    pub heartbeat_interval: u64,
}

/// The response of `GET /gateway`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GatewayInfo {
    /// The WebSocket URL.
    pub url: String,
}

/// The response of `GET /gateway/bot`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct GatewayBotInfo {
    /// The WebSocket URL.
    pub url: String,
    /// The recommended number of shards.
    pub shards: u32,
    /// The session start limit.
    pub session_start_limit: SessionStartLimit,
}

/// The session start limit of a bot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[non_exhaustive]
pub struct SessionStartLimit {
    /// The number of session starts allowed in the period.
    pub total: u32,
    /// The number of session starts left.
    pub remaining: u32,
    /// The time until the limit resets, in milliseconds.
    pub reset_after: u64,
    /// The number of Identify requests allowed for each 5 seconds.
    pub max_concurrency: u32,
}

crate::discord_enum! {
    /// A close code of the gateway.
    pub enum CloseCode: u16 {
        /// An unknown error. Reconnect.
        UnknownError = 4000,
        /// An unknown opcode or a bad payload. Reconnect.
        UnknownOpcode = 4001,
        /// A payload that cannot be decoded, or larger than 4096 bytes. Reconnect.
        DecodeError = 4002,
        /// A payload before Identify. Reconnect.
        NotAuthenticated = 4003,
        /// The token is not valid. Do not reconnect.
        AuthenticationFailed = 4004,
        /// Identify was sent twice. Reconnect.
        AlreadyAuthenticated = 4005,
        /// The sequence of Resume is not valid. Start a new session.
        InvalidSequence = 4007,
        /// Too many payloads. Reconnect.
        RateLimited = 4008,
        /// The session timed out. Start a new session.
        SessionTimedOut = 4009,
        /// The shard is not valid. Do not reconnect.
        InvalidShard = 4010,
        /// The bot needs sharding. Do not reconnect.
        ShardingRequired = 4011,
        /// The API version is not valid. Do not reconnect.
        InvalidApiVersion = 4012,
        /// The intents are not valid. Do not reconnect.
        InvalidIntents = 4013,
        /// A privileged intent is not approved or not turned on. Do not reconnect.
        DisallowedIntents = 4014,
    }
}

impl CloseCode {
    /// Tells if the client can connect again after this code.
    #[must_use]
    pub const fn can_reconnect(self) -> bool {
        !matches!(
            self,
            Self::AuthenticationFailed
                | Self::InvalidShard
                | Self::ShardingRequired
                | Self::InvalidApiVersion
                | Self::InvalidIntents
                | Self::DisallowedIntents
        )
    }

    /// Tells if the client must start a new session (Identify) and not resume.
    #[must_use]
    pub const fn needs_new_session(self) -> bool {
        matches!(self, Self::InvalidSequence | Self::SessionTimedOut)
    }
}

crate::discord_flags! {
    /// Gateway intents: the groups of events that a session receives.
    pub struct Intents: u64 as integer {
        /// Guilds, roles, channels, threads, stage instances, and voice channel status.
        const GUILDS = 1 << 0;
        /// Members (privileged).
        const GUILD_MEMBERS = 1 << 1;
        /// Bans and audit log entries.
        const GUILD_MODERATION = 1 << 2;
        /// Emojis, stickers, and soundboard sounds.
        const GUILD_EXPRESSIONS = 1 << 3;
        /// Integrations.
        const GUILD_INTEGRATIONS = 1 << 4;
        /// Webhooks.
        const GUILD_WEBHOOKS = 1 << 5;
        /// Invites.
        const GUILD_INVITES = 1 << 6;
        /// Voice states and voice channel effects.
        const GUILD_VOICE_STATES = 1 << 7;
        /// Presences (privileged).
        const GUILD_PRESENCES = 1 << 8;
        /// Messages in guilds.
        const GUILD_MESSAGES = 1 << 9;
        /// Reactions in guilds.
        const GUILD_MESSAGE_REACTIONS = 1 << 10;
        /// Typing in guilds.
        const GUILD_MESSAGE_TYPING = 1 << 11;
        /// Messages in DMs.
        const DIRECT_MESSAGES = 1 << 12;
        /// Reactions in DMs.
        const DIRECT_MESSAGE_REACTIONS = 1 << 13;
        /// Typing in DMs.
        const DIRECT_MESSAGE_TYPING = 1 << 14;
        /// The content of messages (privileged). It gives no events.
        const MESSAGE_CONTENT = 1 << 15;
        /// Scheduled events.
        const GUILD_SCHEDULED_EVENTS = 1 << 16;
        /// Auto moderation rules.
        const AUTO_MODERATION_CONFIGURATION = 1 << 20;
        /// Auto moderation action executions.
        const AUTO_MODERATION_EXECUTION = 1 << 21;
        /// Poll votes in guilds.
        const GUILD_MESSAGE_POLLS = 1 << 24;
        /// Poll votes in DMs.
        const DIRECT_MESSAGE_POLLS = 1 << 25;
    }
}

impl Intents {
    /// The privileged intents. They need approval in the Developer Portal.
    pub const PRIVILEGED: Self = Self::GUILD_MEMBERS
        .union(Self::GUILD_PRESENCES)
        .union(Self::MESSAGE_CONTENT);

    /// All intents that are not privileged.
    pub const NON_PRIVILEGED: Self = Self::all().difference(Self::PRIVILEGED);

    /// Gives the privileged intents in this set.
    #[must_use]
    pub const fn privileged(self) -> Self {
        self.intersection(Self::PRIVILEGED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_raw_payload_without_decoding_data() {
        let text = r#"{"op":0,"s":42,"t":"MESSAGE_CREATE","d":{"big":"payload"}}"#;

        let payload: RawPayload<'_> = serde_json::from_str(text).unwrap();

        assert_eq!(payload.op, OpCode::Dispatch);
        assert_eq!(payload.s, Some(42));
        assert_eq!(payload.t, Some("MESSAGE_CREATE"));
        assert_eq!(payload.d.unwrap().get(), r#"{"big":"payload"}"#);
    }

    #[test]
    fn close_codes_tell_what_to_do() {
        assert!(CloseCode::UnknownError.can_reconnect());
        assert!(!CloseCode::DisallowedIntents.can_reconnect());
        assert!(CloseCode::SessionTimedOut.needs_new_session());
        assert!(CloseCode::Unknown(4999).can_reconnect());
    }

    #[test]
    fn privileged_intents() {
        let intents = Intents::GUILDS | Intents::MESSAGE_CONTENT;

        assert_eq!(intents.privileged(), Intents::MESSAGE_CONTENT);
        assert!(!Intents::NON_PRIVILEGED.intersects(Intents::PRIVILEGED));
    }
}
