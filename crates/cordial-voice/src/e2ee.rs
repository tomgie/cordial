//! The interface between the voice connection and end-to-end encryption (DAVE).
//!
//! The voice connection gives the DAVE messages of the voice gateway to an [`E2eeSession`],
//! sends the messages that it gives back, and passes each Opus frame through it.

use cordial_model::id::{ChannelId, UserId};

/// A message that the E2EE session sends over the voice gateway.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum E2eeOutgoing {
    /// A JSON payload: `{"op": op, "d": data}`.
    Json {
        /// The opcode.
        op: u8,
        /// The data.
        data: serde_json::Value,
    },
    /// A binary payload: the opcode byte, then the payload. A client sends no sequence number.
    Binary {
        /// The opcode.
        op: u8,
        /// The payload after the opcode byte.
        payload: Vec<u8>,
    },
}

/// An error of an E2EE session.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum E2eeError {
    /// A protocol message is not valid.
    #[error("E2EE protocol error: {0}")]
    Protocol(String),
    /// A frame cannot be decrypted.
    #[error("cannot decrypt the frame: {0}")]
    Decrypt(String),
    /// A frame cannot be encrypted.
    #[error("cannot encrypt the frame: {0}")]
    Encrypt(String),
}

/// End-to-end encryption of voice frames.
pub trait E2eeSession: Send + 'static {
    /// Gives the highest DAVE protocol version that the session supports. Identify sends it
    /// as `max_dave_protocol_version`. 0 means no E2EE.
    fn max_protocol_version(&self) -> u16;

    /// Reports the Session Description (voice opcode 4) with its DAVE protocol version.
    fn session_description(
        &mut self,
        protocol_version: u16,
        own_user_id: UserId,
        channel_id: ChannelId,
    ) -> Result<Vec<E2eeOutgoing>, E2eeError>;

    /// Reports users that joined the call (voice opcode 11).
    fn clients_connect(&mut self, user_ids: &[UserId]);

    /// Reports a user that left the call (voice opcode 13).
    fn client_disconnect(&mut self, user_id: UserId);

    /// Handles a JSON DAVE message (voice opcodes 21, 22, 24).
    fn json_message(&mut self, op: u8, data: &serde_json::Value) -> Result<Vec<E2eeOutgoing>, E2eeError>;

    /// Handles a binary DAVE message (voice opcodes 25, 27, 29, 30). `payload` has no
    /// sequence number and no opcode byte.
    fn binary_message(&mut self, op: u8, payload: &[u8]) -> Result<Vec<E2eeOutgoing>, E2eeError>;

    /// Encrypts an Opus frame of the own user. Gives the frame unchanged when E2EE is off.
    fn encrypt(&mut self, opus_frame: &[u8]) -> Result<Vec<u8>, E2eeError>;

    /// Decrypts an Opus frame of a sender. Gives the frame unchanged when it is not
    /// encrypted (the silence frame `F8 FF FE`, or E2EE off).
    fn decrypt(&mut self, sender: UserId, frame: &[u8]) -> Result<Vec<u8>, E2eeError>;
}

/// A session without E2EE.
///
/// Since 2026-03, Discord closes voice connections without DAVE with code 4017, except in
/// stage channels. Use it only for stage channels and for tests.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct NoE2ee;

impl E2eeSession for NoE2ee {
    fn max_protocol_version(&self) -> u16 {
        0
    }

    fn session_description(
        &mut self,
        _protocol_version: u16,
        _own_user_id: UserId,
        _channel_id: ChannelId,
    ) -> Result<Vec<E2eeOutgoing>, E2eeError> {
        Ok(Vec::new())
    }

    fn clients_connect(&mut self, _user_ids: &[UserId]) {}

    fn client_disconnect(&mut self, _user_id: UserId) {}

    fn json_message(&mut self, _op: u8, _data: &serde_json::Value) -> Result<Vec<E2eeOutgoing>, E2eeError> {
        Ok(Vec::new())
    }

    fn binary_message(&mut self, _op: u8, _payload: &[u8]) -> Result<Vec<E2eeOutgoing>, E2eeError> {
        Ok(Vec::new())
    }

    fn encrypt(&mut self, opus_frame: &[u8]) -> Result<Vec<u8>, E2eeError> {
        Ok(opus_frame.to_vec())
    }

    fn decrypt(&mut self, _sender: UserId, frame: &[u8]) -> Result<Vec<u8>, E2eeError> {
        Ok(frame.to_vec())
    }
}
