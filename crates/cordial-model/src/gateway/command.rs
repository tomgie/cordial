//! Payloads that a client sends to the gateway.

use serde::{Deserialize, Serialize};

use super::{Intents, OpCode, presence::Activity, presence::Status};
use crate::id::{ChannelId, GuildId, UserId};

/// A payload that a client sends: an opcode and data.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[non_exhaustive]
pub struct SendPayload<T> {
    /// The opcode.
    pub op: OpCode,
    /// The data.
    pub d: T,
}

impl<T> SendPayload<T> {
    /// Makes a payload.
    pub const fn new(op: OpCode, d: T) -> Self {
        Self { op, d }
    }
}

/// The Identify payload (opcode 2): starts a new session.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[non_exhaustive]
pub struct Identify {
    /// The bot token, without the `Bot ` prefix.
    pub token: crate::Secret,
    /// The connection properties.
    pub properties: IdentifyProperties,
    /// Payload compression. Not with transport compression.
    #[serde(skip_serializing_if = "crate::serde_util::is_false")]
    pub compress: bool,
    /// The member count (50 to 250) above which Discord does not send offline members.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub large_threshold: Option<u8>,
    /// The shard ID and the shard count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shard: Option<[u32; 2]>,
    /// The initial presence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence: Option<UpdatePresence>,
    /// The intents.
    pub intents: Intents,
    /// The capabilities.
    #[serde(skip_serializing_if = "crate::serde_util::is_default")]
    pub capabilities: Capabilities,
}

impl Identify {
    /// Makes an Identify payload for one shard.
    #[must_use]
    pub fn new(
        token: impl Into<String>,
        properties: IdentifyProperties,
        intents: Intents,
        shard: [u32; 2],
    ) -> Self {
        Self {
            token: crate::Secret::new(token),
            properties,
            compress: false,
            large_threshold: None,
            shard: Some(shard),
            presence: None,
            intents,
            capabilities: Capabilities::empty(),
        }
    }

    /// Sets the member count above which Discord does not send offline members (50 to 250).
    #[must_use]
    pub fn large_threshold(mut self, threshold: Option<u8>) -> Self {
        self.large_threshold = threshold;
        self
    }

    /// Sets the initial presence.
    #[must_use]
    pub fn presence(mut self, presence: Option<UpdatePresence>) -> Self {
        self.presence = presence;
        self
    }

    /// Sets the capabilities.
    #[must_use]
    pub fn capabilities(mut self, capabilities: Capabilities) -> Self {
        self.capabilities = capabilities;
        self
    }
}

/// The connection properties of Identify.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct IdentifyProperties {
    /// The operating system.
    pub os: String,
    /// The library name.
    pub browser: String,
    /// The library name.
    pub device: String,
}

impl IdentifyProperties {
    /// Makes properties with the current operating system and a library name.
    #[must_use]
    pub fn new(library: &str) -> Self {
        Self {
            os: std::env::consts::OS.to_owned(),
            browser: library.to_owned(),
            device: library.to_owned(),
        }
    }
}

crate::discord_flags! {
    /// Capabilities of a gateway client. They are separate from intents.
    pub struct Capabilities: u64 as integer {
        /// Receive obfuscated channels. Temporary: mandatory for all bots from 2026-11-16.
        const CHANNEL_OBFUSCATION = 1 << 15;
    }
}

/// The Resume payload (opcode 6): continues a session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Resume {
    /// The bot token.
    pub token: crate::Secret,
    /// The session ID from `READY`.
    pub session_id: String,
    /// The last sequence number that was received.
    pub seq: u64,
}

impl Resume {
    /// Makes a Resume payload.
    #[must_use]
    pub fn new(token: impl Into<String>, session_id: impl Into<String>, seq: u64) -> Self {
        Self {
            token: crate::Secret::new(token),
            session_id: session_id.into(),
            seq,
        }
    }
}

/// The Update Presence payload (opcode 3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UpdatePresence {
    /// When the client became idle, in Unix milliseconds, or `None`.
    pub since: Option<u64>,
    /// The activities.
    pub activities: Vec<Activity>,
    /// The status.
    pub status: Status,
    /// Tells if the client is AFK.
    pub afk: bool,
}

impl UpdatePresence {
    /// Makes a presence with a status and activities.
    #[must_use]
    pub fn new(status: Status, activities: Vec<Activity>) -> Self {
        Self {
            since: None,
            activities,
            status,
            afk: false,
        }
    }
}

/// The Update Voice State payload (opcode 4): joins, moves, or leaves a voice channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
pub struct UpdateVoiceState {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The channel to join, or `None` to leave.
    pub channel_id: Option<ChannelId>,
    /// Tells if the client is muted.
    pub self_mute: bool,
    /// Tells if the client is deafened.
    pub self_deaf: bool,
}

impl UpdateVoiceState {
    /// Makes a payload that joins or leaves a voice channel.
    #[must_use]
    pub const fn new(
        guild_id: GuildId,
        channel_id: Option<ChannelId>,
        self_mute: bool,
        self_deaf: bool,
    ) -> Self {
        Self {
            guild_id,
            channel_id,
            self_mute,
            self_deaf,
        }
    }
}

/// The Request Guild Members payload (opcode 8).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RequestGuildMembers {
    /// The guild ID.
    pub guild_id: GuildId,
    /// A prefix of usernames, or empty for all members. Not with `user_ids`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// The maximum number of members. 0 with an empty query means all members.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Tells if presences are included. Needs `GUILD_PRESENCES`.
    #[serde(skip_serializing_if = "crate::serde_util::is_false")]
    pub presences: bool,
    /// Specific users, up to 100. Not with `query`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_ids: Option<Vec<UserId>>,
    /// A nonce, up to 32 bytes, to match the chunk responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
}

impl RequestGuildMembers {
    /// Requests all members. Needs `GUILD_MEMBERS`. Discord permits 1 such request for each
    /// guild every 30 seconds.
    #[must_use]
    pub fn all(guild_id: GuildId) -> Self {
        Self {
            guild_id,
            query: Some(String::new()),
            limit: Some(0),
            presences: false,
            user_ids: None,
            nonce: None,
        }
    }

    /// Requests members whose username starts with `prefix`, up to `limit` (at most 100).
    #[must_use]
    pub fn query(guild_id: GuildId, prefix: impl Into<String>, limit: u32) -> Self {
        Self {
            guild_id,
            query: Some(prefix.into()),
            limit: Some(limit),
            presences: false,
            user_ids: None,
            nonce: None,
        }
    }

    /// Requests specific users, up to 100.
    #[must_use]
    pub fn users(guild_id: GuildId, user_ids: Vec<UserId>) -> Self {
        Self {
            guild_id,
            query: None,
            limit: None,
            presences: false,
            user_ids: Some(user_ids),
            nonce: None,
        }
    }

    /// Sets the nonce.
    #[must_use]
    pub fn nonce(mut self, nonce: impl Into<String>) -> Self {
        self.nonce = Some(nonce.into());
        self
    }

    /// Sets whether presences are included.
    #[must_use]
    pub fn presences(mut self, presences: bool) -> Self {
        self.presences = presences;
        self
    }

    /// Tells if this is a request for all members, which has a separate rate limit.
    #[must_use]
    pub fn is_all_members(&self) -> bool {
        self.user_ids.is_none() && self.query.as_deref() == Some("") && self.limit == Some(0)
    }
}

/// The Request Soundboard Sounds payload (opcode 31).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RequestSoundboardSounds {
    /// The guilds.
    pub guild_ids: Vec<GuildId>,
}

impl RequestSoundboardSounds {
    /// Makes a request for guilds.
    #[must_use]
    pub fn new(guild_ids: Vec<GuildId>) -> Self {
        Self { guild_ids }
    }
}

/// The Request Channel Info payload (opcode 43).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RequestChannelInfo {
    /// The guild ID.
    pub guild_id: GuildId,
    /// The fields: `status` and `voice_start_time`.
    pub fields: Vec<String>,
}

impl RequestChannelInfo {
    /// Requests the voice status and the voice start time of the channels of a guild.
    #[must_use]
    pub fn voice(guild_id: GuildId) -> Self {
        Self {
            guild_id,
            fields: vec!["status".to_owned(), "voice_start_time".to_owned()],
        }
    }
}
