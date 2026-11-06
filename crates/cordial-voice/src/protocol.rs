//! The payloads of the voice gateway, version 8.
//!
//! See `docs/research/04-voice.md`, section 1, and the official page "Voice Connections".

use std::fmt;

use cordial_model::id::{GuildId, UserId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The voice gateway version that this crate uses.
pub const VOICE_GATEWAY_VERSION: u8 = 8;

/// An opcode of the voice gateway.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VoiceOpCode {
    /// Starts a voice session (client).
    Identify,
    /// Selects the transport protocol and the encryption mode (client).
    SelectProtocol,
    /// Gives the SSRC, the UDP address, and the encryption modes (server).
    Ready,
    /// Keeps the connection alive (client).
    Heartbeat,
    /// Gives the encryption key and the DAVE protocol version (server).
    SessionDescription,
    /// Tells the speaking state (both directions).
    Speaking,
    /// Acknowledges a heartbeat (server).
    HeartbeatAck,
    /// Resumes a session (client).
    Resume,
    /// Gives the heartbeat interval (server).
    Hello,
    /// Confirms a resume (server).
    Resumed,
    /// Tells that users joined the call (server).
    ClientsConnect,
    /// Gives the media SSRCs of a user (server, not documented).
    ClientFlags,
    /// Tells that a user left the call (server).
    ClientDisconnect,
    /// DAVE: prepares a protocol transition (server).
    DavePrepareTransition,
    /// DAVE: executes a protocol transition (server).
    DaveExecuteTransition,
    /// DAVE: the client is ready for a transition (client).
    DaveTransitionReady,
    /// DAVE: prepares a new epoch (server).
    DavePrepareEpoch,
    /// DAVE: the MLS external sender package (server, binary).
    DaveMlsExternalSender,
    /// DAVE: an MLS key package (client, binary).
    DaveMlsKeyPackage,
    /// DAVE: MLS proposals (server, binary).
    DaveMlsProposals,
    /// DAVE: an MLS commit and an optional welcome (client, binary).
    DaveMlsCommitWelcome,
    /// DAVE: announces the winning commit of a transition (server, binary).
    DaveMlsAnnounceCommitTransition,
    /// DAVE: an MLS welcome (server, binary).
    DaveMlsWelcome,
    /// DAVE: the commit or the welcome is not valid (client).
    DaveMlsInvalidCommitWelcome,
}

impl VoiceOpCode {
    /// Gives the opcode for a value. Gives `None` for an unknown value.
    #[must_use]
    pub const fn from_value(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Identify,
            1 => Self::SelectProtocol,
            2 => Self::Ready,
            3 => Self::Heartbeat,
            4 => Self::SessionDescription,
            5 => Self::Speaking,
            6 => Self::HeartbeatAck,
            7 => Self::Resume,
            8 => Self::Hello,
            9 => Self::Resumed,
            11 => Self::ClientsConnect,
            12 => Self::ClientFlags,
            13 => Self::ClientDisconnect,
            21 => Self::DavePrepareTransition,
            22 => Self::DaveExecuteTransition,
            23 => Self::DaveTransitionReady,
            24 => Self::DavePrepareEpoch,
            25 => Self::DaveMlsExternalSender,
            26 => Self::DaveMlsKeyPackage,
            27 => Self::DaveMlsProposals,
            28 => Self::DaveMlsCommitWelcome,
            29 => Self::DaveMlsAnnounceCommitTransition,
            30 => Self::DaveMlsWelcome,
            31 => Self::DaveMlsInvalidCommitWelcome,
            _ => return None,
        })
    }

    /// Gives the value of the opcode.
    #[must_use]
    pub const fn value(self) -> u8 {
        match self {
            Self::Identify => 0,
            Self::SelectProtocol => 1,
            Self::Ready => 2,
            Self::Heartbeat => 3,
            Self::SessionDescription => 4,
            Self::Speaking => 5,
            Self::HeartbeatAck => 6,
            Self::Resume => 7,
            Self::Hello => 8,
            Self::Resumed => 9,
            Self::ClientsConnect => 11,
            Self::ClientFlags => 12,
            Self::ClientDisconnect => 13,
            Self::DavePrepareTransition => 21,
            Self::DaveExecuteTransition => 22,
            Self::DaveTransitionReady => 23,
            Self::DavePrepareEpoch => 24,
            Self::DaveMlsExternalSender => 25,
            Self::DaveMlsKeyPackage => 26,
            Self::DaveMlsProposals => 27,
            Self::DaveMlsCommitWelcome => 28,
            Self::DaveMlsAnnounceCommitTransition => 29,
            Self::DaveMlsWelcome => 30,
            Self::DaveMlsInvalidCommitWelcome => 31,
        }
    }

    /// Tells if the server sends this opcode as a binary message.
    #[must_use]
    pub const fn is_binary(self) -> bool {
        matches!(
            self,
            Self::DaveMlsExternalSender
                | Self::DaveMlsKeyPackage
                | Self::DaveMlsProposals
                | Self::DaveMlsCommitWelcome
                | Self::DaveMlsAnnounceCommitTransition
                | Self::DaveMlsWelcome
        )
    }
}

/// The Identify payload (opcode 0).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Identify {
    /// The guild ID.
    pub server_id: GuildId,
    /// The user ID of the bot.
    pub user_id: UserId,
    /// The session ID from `VOICE_STATE_UPDATE`.
    pub session_id: String,
    /// The token from `VOICE_SERVER_UPDATE`.
    pub token: String,
    /// The highest DAVE protocol version that the client supports. 0 means no DAVE.
    pub max_dave_protocol_version: u16,
}

impl Identify {
    /// Makes an Identify payload.
    #[must_use]
    pub fn new(
        server_id: GuildId,
        user_id: UserId,
        session_id: String,
        token: String,
        max_dave_protocol_version: u16,
    ) -> Self {
        Self {
            server_id,
            user_id,
            session_id,
            token,
            max_dave_protocol_version,
        }
    }
}

impl fmt::Debug for Identify {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Identify")
            .field("server_id", &self.server_id)
            .field("user_id", &self.user_id)
            .field("session_id", &self.session_id)
            .field("max_dave_protocol_version", &self.max_dave_protocol_version)
            .finish_non_exhaustive()
    }
}

/// The Select Protocol payload (opcode 1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SelectProtocol {
    /// The transport protocol. Always `udp`.
    pub protocol: String,
    /// The address and the encryption mode.
    pub data: SelectProtocolData,
}

impl SelectProtocol {
    /// Makes a payload for UDP with the external address from IP discovery.
    #[must_use]
    pub fn udp(address: String, port: u16, mode: EncryptionMode) -> Self {
        Self {
            protocol: "udp".to_owned(),
            data: SelectProtocolData {
                address,
                port,
                mode,
            },
        }
    }
}

/// The data of Select Protocol.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SelectProtocolData {
    /// The external IP address of the client.
    pub address: String,
    /// The external UDP port of the client.
    pub port: u16,
    /// The encryption mode.
    pub mode: EncryptionMode,
}

/// The Ready payload (opcode 2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Ready {
    /// The SSRC of the client.
    pub ssrc: u32,
    /// The IP address of the voice server.
    pub ip: String,
    /// The UDP port of the voice server.
    pub port: u16,
    /// The encryption modes that the server supports. Unknown modes stay as text.
    pub modes: Vec<String>,
}

/// The Session Description payload (opcode 4).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct SessionDescription {
    /// The encryption mode.
    pub mode: String,
    /// The 32-byte key of the transport encryption.
    pub secret_key: Vec<u8>,
    /// The initial DAVE protocol version. 0 means no E2EE.
    #[serde(default)]
    pub dave_protocol_version: u16,
}

impl fmt::Debug for SessionDescription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionDescription")
            .field("mode", &self.mode)
            .field("dave_protocol_version", &self.dave_protocol_version)
            .finish_non_exhaustive()
    }
}

/// The Speaking payload (opcode 5).
///
/// The client sends it before audio. The server sends it for other users: then `user_id`
/// is set, and the payload maps the SSRC to the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Speaking {
    /// The speaking flags.
    pub speaking: SpeakingFlags,
    /// The delay. Bots use 0.
    #[serde(default)]
    pub delay: u32,
    /// The SSRC.
    pub ssrc: u32,
    /// The user. Only the server sets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<UserId>,
}

impl Speaking {
    /// Makes the payload that the client sends.
    #[must_use]
    pub const fn new(speaking: SpeakingFlags, ssrc: u32) -> Self {
        Self {
            speaking,
            delay: 0,
            ssrc,
            user_id: None,
        }
    }
}

/// The speaking flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SpeakingFlags(u32);

impl SpeakingFlags {
    /// No flags: not speaking.
    pub const NONE: Self = Self(0);
    /// Normal transmission of voice audio.
    pub const MICROPHONE: Self = Self(1 << 0);
    /// Transmission of context audio for video, without the speaking indicator.
    pub const SOUNDSHARE: Self = Self(1 << 1);
    /// Priority speaker: the audio of other speakers becomes lower.
    pub const PRIORITY: Self = Self(1 << 2);

    /// Makes flags from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// Gives the raw bits.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Tells if all flags of `other` are set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Tells if no flag is set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for SpeakingFlags {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

impl Serialize for SpeakingFlags {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for SpeakingFlags {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u32::deserialize(deserializer).map(Self)
    }
}

/// The Heartbeat payload of version 8 (opcode 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Heartbeat {
    /// The nonce. The acknowledgement gives it back.
    pub t: u64,
    /// The last sequence number that the client received, or -1.
    pub seq_ack: i64,
}

impl Heartbeat {
    /// Makes a heartbeat payload.
    #[must_use]
    pub const fn new(t: u64, seq_ack: i64) -> Self {
        Self { t, seq_ack }
    }
}

/// The Heartbeat ACK payload of version 8 (opcode 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct HeartbeatAck {
    /// The nonce of the heartbeat.
    pub t: u64,
}

/// The Resume payload of version 8 (opcode 7).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Resume {
    /// The guild ID.
    pub server_id: GuildId,
    /// The session ID.
    pub session_id: String,
    /// The token.
    pub token: String,
    /// The last sequence number that the client received, or -1.
    pub seq_ack: i64,
}

impl Resume {
    /// Makes a Resume payload.
    #[must_use]
    pub fn new(server_id: GuildId, session_id: String, token: String, seq_ack: i64) -> Self {
        Self {
            server_id,
            session_id,
            token,
            seq_ack,
        }
    }
}

impl fmt::Debug for Resume {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Resume")
            .field("server_id", &self.server_id)
            .field("session_id", &self.session_id)
            .field("seq_ack", &self.seq_ack)
            .finish_non_exhaustive()
    }
}

/// The Hello payload (opcode 8).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Hello {
    /// The heartbeat interval in milliseconds. It can have a fraction.
    pub heartbeat_interval: f64,
}

/// The Clients Connect payload (opcode 11).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ClientsConnect {
    /// The users that joined.
    pub user_ids: Vec<UserId>,
}

/// The Client Disconnect payload (opcode 13).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ClientDisconnect {
    /// The user that left.
    pub user_id: UserId,
}

/// The Client Flags payload (opcode 12). It is not documented. It gives the audio SSRC of
/// a user.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ClientSsrc {
    /// The user.
    pub user_id: UserId,
    /// The audio SSRC.
    #[serde(default)]
    pub audio_ssrc: u32,
}

/// A transport encryption mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum EncryptionMode {
    /// `aead_aes256_gcm_rtpsize`: AES-256-GCM. Preferred when the server offers it.
    Aes256GcmRtpSize,
    /// `aead_xchacha20_poly1305_rtpsize`: XChaCha20-Poly1305. The server always offers it.
    XChaCha20Poly1305RtpSize,
}

impl EncryptionMode {
    /// Gives the name that the voice gateway uses.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Aes256GcmRtpSize => "aead_aes256_gcm_rtpsize",
            Self::XChaCha20Poly1305RtpSize => "aead_xchacha20_poly1305_rtpsize",
        }
    }

    /// Gives the mode for a name. Gives `None` for a mode that this crate does not support.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "aead_aes256_gcm_rtpsize" => Some(Self::Aes256GcmRtpSize),
            "aead_xchacha20_poly1305_rtpsize" => Some(Self::XChaCha20Poly1305RtpSize),
            _ => None,
        }
    }

    /// Selects a mode from the modes of Ready: AES-256-GCM when offered, else XChaCha20.
    #[must_use]
    pub fn select<S: AsRef<str>>(offered: &[S]) -> Option<Self> {
        let offers = |mode: Self| offered.iter().any(|name| name.as_ref() == mode.name());
        [Self::Aes256GcmRtpSize, Self::XChaCha20Poly1305RtpSize]
            .into_iter()
            .find(|mode| offers(*mode))
    }
}

impl Serialize for EncryptionMode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for EncryptionMode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Self::from_name(&name)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown encryption mode {name}")))
    }
}

/// What the client does after the voice gateway closes the connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CloseAction {
    /// Connect again and resume the session.
    Resume,
    /// Connect again and identify with a new session.
    NewSession,
    /// Do not connect again.
    Stop,
}

/// A close code of the voice gateway.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VoiceCloseCode {
    /// 4001: the client sent an unknown opcode.
    UnknownOpcode,
    /// 4002: the server cannot decode a payload.
    DecodeFailure,
    /// 4003: the client sent a payload before Identify.
    NotAuthenticated,
    /// 4004: the token in Identify is not correct.
    AuthenticationFailed,
    /// 4005: the client sent more than one Identify.
    AlreadyAuthenticated,
    /// 4006: the session is no longer valid.
    SessionNoLongerValid,
    /// 4009: the session timed out.
    SessionTimeout,
    /// 4011: the server for the connection was not found.
    ServerNotFound,
    /// 4012: the server does not know the protocol in Select Protocol.
    UnknownProtocol,
    /// 4014: the client was disconnected: kicked, the channel was deleted, or the client moved.
    Disconnected,
    /// 4015: the voice server crashed. Resume.
    VoiceServerCrashed,
    /// 4016: the server does not know the encryption mode.
    UnknownEncryptionMode,
    /// 4017: the channel needs DAVE end-to-end encryption.
    E2eeRequired,
    /// 4020: the client sent a malformed request.
    BadRequest,
    /// 4021: the client is rate limited.
    RateLimited,
    /// 4022: the call was terminated, for example because the channel was deleted.
    CallTerminated,
    /// A code that this crate does not know, for example a WebSocket code.
    Other(u16),
}

impl VoiceCloseCode {
    /// Gives the close code for a value.
    #[must_use]
    pub const fn from_value(value: u16) -> Self {
        match value {
            4001 => Self::UnknownOpcode,
            4002 => Self::DecodeFailure,
            4003 => Self::NotAuthenticated,
            4004 => Self::AuthenticationFailed,
            4005 => Self::AlreadyAuthenticated,
            4006 => Self::SessionNoLongerValid,
            4009 => Self::SessionTimeout,
            4011 => Self::ServerNotFound,
            4012 => Self::UnknownProtocol,
            4014 => Self::Disconnected,
            4015 => Self::VoiceServerCrashed,
            4016 => Self::UnknownEncryptionMode,
            4017 => Self::E2eeRequired,
            4020 => Self::BadRequest,
            4021 => Self::RateLimited,
            4022 => Self::CallTerminated,
            other => Self::Other(other),
        }
    }

    /// Gives the value of the close code.
    #[must_use]
    pub const fn value(self) -> u16 {
        match self {
            Self::UnknownOpcode => 4001,
            Self::DecodeFailure => 4002,
            Self::NotAuthenticated => 4003,
            Self::AuthenticationFailed => 4004,
            Self::AlreadyAuthenticated => 4005,
            Self::SessionNoLongerValid => 4006,
            Self::SessionTimeout => 4009,
            Self::ServerNotFound => 4011,
            Self::UnknownProtocol => 4012,
            Self::Disconnected => 4014,
            Self::VoiceServerCrashed => 4015,
            Self::UnknownEncryptionMode => 4016,
            Self::E2eeRequired => 4017,
            Self::BadRequest => 4020,
            Self::RateLimited => 4021,
            Self::CallTerminated => 4022,
            Self::Other(value) => value,
        }
    }

    /// Gives what the client does after this close code.
    ///
    /// - 4015 and unknown codes: resume.
    /// - 4006 and 4009: identify with a new session.
    /// - All other known codes: stop. A new connection gives the same result, or Discord
    ///   tells the client not to connect again (4014, 4021, 4022).
    #[must_use]
    pub const fn action(self) -> CloseAction {
        match self {
            Self::VoiceServerCrashed | Self::Other(_) => CloseAction::Resume,
            Self::SessionNoLongerValid | Self::SessionTimeout => CloseAction::NewSession,
            _ => CloseAction::Stop,
        }
    }

    /// Gives a description of the close code.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::UnknownOpcode => "the client sent an unknown opcode",
            Self::DecodeFailure => "the server cannot decode a payload of the client",
            Self::NotAuthenticated => "the client sent a payload before Identify",
            Self::AuthenticationFailed => "the token is not correct",
            Self::AlreadyAuthenticated => "the client sent more than one Identify",
            Self::SessionNoLongerValid => "the session is no longer valid",
            Self::SessionTimeout => "the session timed out",
            Self::ServerNotFound => "the voice server was not found",
            Self::UnknownProtocol => "the server does not know the protocol",
            Self::Disconnected => {
                "the client was disconnected: kicked, moved, or the channel was deleted"
            }
            Self::VoiceServerCrashed => "the voice server crashed",
            Self::UnknownEncryptionMode => "the server does not know the encryption mode",
            Self::E2eeRequired => "the channel needs DAVE end-to-end encryption",
            Self::BadRequest => "the client sent a malformed request",
            Self::RateLimited => "the client is rate limited",
            Self::CallTerminated => "the call was terminated",
            Self::Other(_) => "an unknown close code",
        }
    }
}

impl fmt::Display for VoiceCloseCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} ({})", self.value(), self.description())
    }
}

/// A JSON message of the voice gateway, for receive.
#[derive(Debug, Deserialize)]
pub(crate) struct ReceivedPayload {
    pub(crate) op: u8,
    #[serde(default)]
    pub(crate) d: serde_json::Value,
    #[serde(default)]
    pub(crate) seq: Option<i64>,
}

/// A JSON message of the voice gateway, for send.
#[derive(Serialize)]
pub(crate) struct SentPayload<'a, T: Serialize> {
    pub(crate) op: u8,
    pub(crate) d: &'a T,
}

/// Encodes a JSON message.
pub(crate) fn encode<T: Serialize>(op: VoiceOpCode, data: &T) -> String {
    let payload = SentPayload {
        op: op.value(),
        d: data,
    };
    // The payload types contain only strings, numbers, and lists: encode cannot fail.
    serde_json::to_string(&payload).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcodes_round_trip() {
        for value in 0..=40 {
            if let Some(op) = VoiceOpCode::from_value(value) {
                assert_eq!(op.value(), value);
            }
        }
        assert!(VoiceOpCode::DaveMlsWelcome.is_binary());
        assert!(!VoiceOpCode::DaveMlsInvalidCommitWelcome.is_binary());
    }

    #[test]
    fn mode_selection_prefers_aes() {
        let both = ["aead_xchacha20_poly1305_rtpsize", "aead_aes256_gcm_rtpsize"];
        assert_eq!(
            EncryptionMode::select(&both),
            Some(EncryptionMode::Aes256GcmRtpSize)
        );
        let one = [
            "xsalsa20_poly1305_lite_rtpsize",
            "aead_xchacha20_poly1305_rtpsize",
        ];
        assert_eq!(
            EncryptionMode::select(&one),
            Some(EncryptionMode::XChaCha20Poly1305RtpSize)
        );
        assert_eq!(EncryptionMode::select(&["xsalsa20_poly1305"]), None);
    }

    #[test]
    fn close_code_actions() {
        assert_eq!(
            VoiceCloseCode::from_value(4015).action(),
            CloseAction::Resume
        );
        assert_eq!(
            VoiceCloseCode::from_value(4006).action(),
            CloseAction::NewSession
        );
        assert_eq!(
            VoiceCloseCode::from_value(4009).action(),
            CloseAction::NewSession
        );
        for code in [4014, 4021, 4022, 4004, 4017] {
            assert_eq!(VoiceCloseCode::from_value(code).action(), CloseAction::Stop);
        }
        assert_eq!(
            VoiceCloseCode::from_value(1006).action(),
            CloseAction::Resume
        );
        assert_eq!(VoiceCloseCode::from_value(4022).value(), 4022);
    }

    #[test]
    fn identify_has_dave_version() {
        let identify = Identify::new(
            GuildId::new_const(41_771_983_423_143_937),
            UserId::new_const(104_694_319_306_248_192),
            "my_session_id".to_owned(),
            "my_token".to_owned(),
            1,
        );
        let text = encode(VoiceOpCode::Identify, &identify);
        let value: serde_json::Value = serde_json::from_str(&text).expect("valid JSON");
        assert_eq!(
            value,
            serde_json::json!({
                "op": 0,
                "d": {
                    "server_id": "41771983423143937",
                    "user_id": "104694319306248192",
                    "session_id": "my_session_id",
                    "token": "my_token",
                    "max_dave_protocol_version": 1
                }
            })
        );
    }

    #[test]
    fn speaking_from_server_has_user() {
        let speaking: Speaking =
            serde_json::from_str(r#"{"user_id":"5","ssrc":110,"speaking":1}"#).expect("valid");
        assert_eq!(speaking.user_id, Some(UserId::new_const(5)));
        assert_eq!(speaking.ssrc, 110);
        assert!(speaking.speaking.contains(SpeakingFlags::MICROPHONE));

        let sent = serde_json::to_value(Speaking::new(
            SpeakingFlags::MICROPHONE | SpeakingFlags::PRIORITY,
            1,
        ))
        .expect("valid");
        assert_eq!(
            sent,
            serde_json::json!({"speaking": 5, "delay": 0, "ssrc": 1})
        );
    }
}
