//! DAVE: the end-to-end encryption of Discord audio and video, protocol version 1.
//!
//! [`DaveSession`] implements [`E2eeSession`]. It keeps an MLS group (RFC 9420) with the other
//! members of the call. The voice gateway is the MLS delivery service and the external sender.
//! Each member derives a key ratchet for each sender from the group, and encrypts each Opus frame
//! with AES-128-GCM.
//!
//! The behavior follows the DAVE protocol whitepaper and libdave (`discord/libdave`):
//!
//! | Module | Content | libdave source |
//! |---|---|---|
//! | `frame` | The protocol frame format and ULEB128 | `frame_processors.cpp`, `utils/leb128.cpp` |
//! | `cryptor` | AES-128-GCM with an 8-byte tag | `openssl_cryptor.cpp`, `common.h` |
//! | `ratchet` | The per-sender hash ratchet | `mls_key_ratchet.cpp`, mlspp `HashRatchet` |
//! | `encryptor` | The send side | `encryptor.cpp` |
//! | `decryptor` | The receive side, replay checks, expiry | `decryptor.cpp`, `cryptor_manager.cpp` |
//! | `mls` | The MLS group on OpenMLS | `mls/session.cpp`, `mls/parameters.cpp` |
//! | `provider` | The OpenMLS provider for ciphersuite 2 | `mls/parameters.cpp` |
//!
//! The flow of the voice gateway opcodes follows the libdave sample `DaveSessionManager.ts`.

mod cryptor;
mod decryptor;
mod encryptor;
mod frame;
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
pub mod fuzzing;
mod mls;
mod provider;
mod ratchet;
#[cfg(any(test, feature = "test-gateway"))]
#[doc(hidden)]
pub mod test_gateway;

#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::time::Instant;

use cordial_model::id::{ChannelId, UserId};
use serde_json::{Value, json};

use self::decryptor::Decryptor;
use self::encryptor::Encryptor;
use self::mls::{CommitOutcome, MlsSession};
use crate::e2ee::{E2eeError, E2eeOutgoing, E2eeSession};

/// The DAVE protocol version that this implementation supports.
const PROTOCOL_VERSION: u16 = 1;

/// The transition ID that executes at once (libdave `kInitTransitionId`).
const INIT_TRANSITION_ID: u16 = 0;

/// The epoch that tells the client to make a new group (libdave sample
/// `MLS_NEW_GROUP_EXPECTED_EPOCH`).
const NEW_GROUP_EPOCH: u64 = 1;

/// Voice gateway opcodes of DAVE.
mod op {
    pub(super) const PREPARE_TRANSITION: u8 = 21;
    pub(super) const EXECUTE_TRANSITION: u8 = 22;
    pub(super) const READY_FOR_TRANSITION: u8 = 23;
    pub(super) const PREPARE_EPOCH: u8 = 24;
    pub(super) const EXTERNAL_SENDER: u8 = 25;
    pub(super) const KEY_PACKAGE: u8 = 26;
    pub(super) const PROPOSALS: u8 = 27;
    pub(super) const COMMIT_WELCOME: u8 = 28;
    pub(super) const ANNOUNCE_COMMIT_TRANSITION: u8 = 29;
    pub(super) const WELCOME: u8 = 30;
    pub(super) const INVALID_COMMIT_WELCOME: u8 = 31;
}

fn protocol_error(message: impl fmt::Display) -> E2eeError {
    E2eeError::Protocol(message.to_string())
}

/// Reads an unsigned integer field of a JSON payload. The field can be a number or a string.
fn json_u64(data: &Value, key: &str) -> Result<u64, E2eeError> {
    let value = data
        .get(key)
        .ok_or_else(|| protocol_error(format!("the field `{key}` is missing")))?;
    let number = match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    };
    number.ok_or_else(|| protocol_error(format!("the field `{key}` is not an unsigned integer")))
}

fn json_u16(data: &Value, key: &str) -> Result<u16, E2eeError> {
    u16::try_from(json_u64(data, key)?)
        .map_err(|_| protocol_error(format!("the field `{key}` is too large")))
}

/// Splits the `u16` transition ID (big-endian, TLS) from a binary payload.
fn split_transition_id(payload: &[u8]) -> Result<(u16, &[u8]), E2eeError> {
    match payload {
        [high, low, rest @ ..] => Ok((u16::from_be_bytes([*high, *low]), rest)),
        _ => Err(protocol_error("the payload has no transition ID")),
    }
}

fn ready_for_transition(transition_id: u16) -> E2eeOutgoing {
    E2eeOutgoing::Json {
        op: op::READY_FOR_TRANSITION,
        data: json!({ "transition_id": transition_id }),
    }
}

fn invalid_commit_welcome(transition_id: u16) -> E2eeOutgoing {
    E2eeOutgoing::Json {
        op: op::INVALID_COMMIT_WELCOME,
        data: json!({ "transition_id": transition_id }),
    }
}

/// A DAVE end-to-end encryption session for one voice connection.
///
/// Give the session to the voice connection. The connection reports the voice gateway messages
/// through [`E2eeSession`], sends the messages that the session gives back, and passes each Opus
/// frame through [`E2eeSession::encrypt`] and [`E2eeSession::decrypt`].
pub struct DaveSession {
    own_user_id: Option<u64>,
    channel_id: Option<u64>,
    /// The protocol version of the session (from opcode 4 or 24).
    protocol_version: u16,
    mls: MlsSession,
    /// The users in the call, from opcodes 11 and 13.
    connected: HashSet<u64>,
    /// The prepared transitions: transition ID to protocol version.
    transitions: HashMap<u16, u16>,
    /// The protocol version of the latest prepared transition. `None` before the first one.
    latest_prepared_version: Option<u16>,
    encryptor: Encryptor,
    decryptors: HashMap<u64, Decryptor>,
}

impl fmt::Debug for DaveSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DaveSession")
            .field("own_user_id", &self.own_user_id)
            .field("channel_id", &self.channel_id)
            .field("protocol_version", &self.protocol_version)
            .field("mls", &self.mls)
            .field("connected", &self.connected.len())
            .finish_non_exhaustive()
    }
}

impl DaveSession {
    /// Makes a session. The session makes a new MLS signature key pair.
    ///
    /// # Errors
    ///
    /// Gives an error when the key pair cannot be made.
    pub fn new() -> Result<Self, E2eeError> {
        let mls = MlsSession::new().map_err(protocol_error)?;
        Ok(Self {
            own_user_id: None,
            channel_id: None,
            protocol_version: 0,
            mls,
            connected: HashSet::new(),
            transitions: HashMap::new(),
            latest_prepared_version: None,
            encryptor: Encryptor::new(),
            decryptors: HashMap::new(),
        })
    }

    /// Gives the users that can be in the group: the connected users and the own user.
    fn recognized_users(&self) -> HashSet<u64> {
        let mut users = self.connected.clone();
        users.extend(self.own_user_id);
        users
    }

    /// Makes a new group and gives the new key package (opcode 26).
    fn reset_group(&mut self) -> Result<E2eeOutgoing, E2eeError> {
        let (Some(own_user_id), Some(channel_id)) = (self.own_user_id, self.channel_id) else {
            return Err(protocol_error("the session description did not arrive"));
        };
        self.mls
            .init(own_user_id, channel_id)
            .map_err(protocol_error)?;
        self.key_package()
    }

    fn key_package(&mut self) -> Result<E2eeOutgoing, E2eeError> {
        let payload = self.mls.key_package().map_err(protocol_error)?;
        Ok(E2eeOutgoing::Binary {
            op: op::KEY_PACKAGE,
            payload,
        })
    }

    /// Sets up the decryptor of a user for a protocol version.
    fn set_up_decryptor(&mut self, user_id: u64, protocol_version: u16, now: Instant) {
        let ratchet = if protocol_version == 0 {
            None
        } else {
            self.mls.key_ratchet(user_id)
        };
        let decryptor = self
            .decryptors
            .entry(user_id)
            .or_insert_with(|| Decryptor::new(protocol_version == 0));
        decryptor.transition_to_passthrough(protocol_version == 0, now);
        decryptor.transition_to_key_ratchet(ratchet, now);
    }

    /// Prepares the receive side for a transition. A transition with the ID 0 executes at once.
    fn prepare_transition(&mut self, transition_id: u16, protocol_version: u16) {
        let now = Instant::now();
        let others: Vec<u64> = self
            .connected
            .iter()
            .copied()
            .filter(|&user_id| Some(user_id) != self.own_user_id)
            .collect();
        for user_id in others {
            self.set_up_decryptor(user_id, protocol_version, now);
        }

        if transition_id == INIT_TRANSITION_ID {
            self.switch_send_side(protocol_version);
        } else {
            self.transitions.insert(transition_id, protocol_version);
        }
        self.latest_prepared_version = Some(protocol_version);
    }

    /// Gives the protocol version for the receive side of a user that has no transition yet.
    fn receive_version(&self) -> u16 {
        self.latest_prepared_version
            .unwrap_or(self.protocol_version)
    }

    /// Executes a prepared transition on the send side.
    fn execute_transition(&mut self, transition_id: u16) {
        if let Some(protocol_version) = self.transitions.remove(&transition_id) {
            self.switch_send_side(protocol_version);
        }
    }

    fn switch_send_side(&mut self, protocol_version: u16) {
        if protocol_version == 0 {
            self.mls.reset();
            self.encryptor.set_key_ratchet(None);
            self.encryptor.set_passthrough(true);
            return;
        }

        let ratchet = self
            .own_user_id
            .and_then(|user_id| self.mls.key_ratchet(user_id));
        self.encryptor.set_key_ratchet(ratchet);
        self.encryptor.set_passthrough(false);
    }

    /// Gives the messages after the group changed: opcode 23, except for transition 0.
    fn group_changed(&mut self, transition_id: u16) -> Vec<E2eeOutgoing> {
        let protocol_version = self.protocol_version;
        self.prepare_transition(transition_id, protocol_version);
        if transition_id == INIT_TRANSITION_ID {
            Vec::new()
        } else {
            vec![ready_for_transition(transition_id)]
        }
    }

    fn handle_commit(&mut self, payload: &[u8]) -> Result<Vec<E2eeOutgoing>, E2eeError> {
        let (transition_id, commit) = split_transition_id(payload)?;
        match self.mls.process_commit(commit) {
            Ok(CommitOutcome::Applied) => Ok(self.group_changed(transition_id)),
            Ok(CommitOutcome::Ignored | CommitOutcome::Removed) => Ok(Vec::new()),
            Err(cause) => {
                tracing::warn!(transition_id, %cause, "DAVE commit is not valid");
                Ok(vec![
                    invalid_commit_welcome(transition_id),
                    self.reset_group()?,
                ])
            }
        }
    }

    fn handle_welcome(&mut self, payload: &[u8]) -> Result<Vec<E2eeOutgoing>, E2eeError> {
        let (transition_id, welcome) = split_transition_id(payload)?;
        let recognized = self.recognized_users();
        match self.mls.process_welcome(welcome, &recognized) {
            Ok(()) => Ok(self.group_changed(transition_id)),
            Err(cause) => {
                tracing::warn!(transition_id, %cause, "DAVE welcome is not valid");
                // libdave only sends a new key package here. A reset is safe too when no group
                // is established, and it removes a pending state that can be stale.
                let key_package = if self.mls.is_established() {
                    self.key_package()?
                } else {
                    self.reset_group()?
                };
                Ok(vec![invalid_commit_welcome(transition_id), key_package])
            }
        }
    }

    fn handle_proposals(&mut self, payload: &[u8]) -> Result<Vec<E2eeOutgoing>, E2eeError> {
        let recognized = self.recognized_users();
        let commit_welcome = self
            .mls
            .process_proposals(payload, &recognized)
            .map_err(protocol_error)?;
        Ok(commit_welcome
            .map(|payload| E2eeOutgoing::Binary {
                op: op::COMMIT_WELCOME,
                payload,
            })
            .into_iter()
            .collect())
    }
}

impl E2eeSession for DaveSession {
    fn max_protocol_version(&self) -> u16 {
        PROTOCOL_VERSION
    }

    fn session_description(
        &mut self,
        protocol_version: u16,
        own_user_id: UserId,
        channel_id: ChannelId,
    ) -> Result<Vec<E2eeOutgoing>, E2eeError> {
        self.own_user_id = Some(own_user_id.get());
        self.channel_id = Some(channel_id.get());
        self.protocol_version = protocol_version;

        if protocol_version == 0 {
            self.prepare_transition(INIT_TRANSITION_ID, 0);
            return Ok(Vec::new());
        }

        // Do not send plaintext in an end-to-end encrypted call.
        self.encryptor.set_passthrough(false);
        Ok(vec![self.reset_group()?])
    }

    fn clients_connect(&mut self, user_ids: &[UserId]) {
        let now = Instant::now();
        let protocol_version = self.receive_version();
        for user_id in user_ids {
            let user_id = user_id.get();
            self.connected.insert(user_id);
            if Some(user_id) != self.own_user_id {
                self.set_up_decryptor(user_id, protocol_version, now);
            }
        }
    }

    fn client_disconnect(&mut self, user_id: UserId) {
        self.connected.remove(&user_id.get());
        self.decryptors.remove(&user_id.get());
    }

    fn json_message(&mut self, op: u8, data: &Value) -> Result<Vec<E2eeOutgoing>, E2eeError> {
        match op {
            op::PREPARE_TRANSITION => {
                let protocol_version = json_u16(data, "protocol_version")?;
                let transition_id = json_u16(data, "transition_id")?;
                self.prepare_transition(transition_id, protocol_version);
                if transition_id == INIT_TRANSITION_ID {
                    Ok(Vec::new())
                } else {
                    Ok(vec![ready_for_transition(transition_id)])
                }
            }
            op::EXECUTE_TRANSITION => {
                let transition_id = json_u16(data, "transition_id")?;
                self.execute_transition(transition_id);
                Ok(Vec::new())
            }
            op::PREPARE_EPOCH => {
                let protocol_version = json_u16(data, "protocol_version")?;
                let epoch = json_u64(data, "epoch")?;
                if epoch != NEW_GROUP_EPOCH {
                    return Ok(Vec::new());
                }
                self.protocol_version = protocol_version;
                Ok(vec![self.reset_group()?])
            }
            other => Err(protocol_error(format!("unknown DAVE JSON opcode {other}"))),
        }
    }

    fn binary_message(&mut self, op: u8, payload: &[u8]) -> Result<Vec<E2eeOutgoing>, E2eeError> {
        match op {
            op::EXTERNAL_SENDER => {
                self.mls
                    .set_external_sender(payload)
                    .map_err(protocol_error)?;
                Ok(Vec::new())
            }
            op::PROPOSALS => self.handle_proposals(payload),
            op::ANNOUNCE_COMMIT_TRANSITION => self.handle_commit(payload),
            op::WELCOME => self.handle_welcome(payload),
            other => Err(protocol_error(format!(
                "unknown DAVE binary opcode {other}"
            ))),
        }
    }

    fn can_encrypt(&self) -> bool {
        self.encryptor.can_encrypt()
    }

    fn encrypt(&mut self, opus_frame: &[u8]) -> Result<Vec<u8>, E2eeError> {
        if opus_frame == frame::OPUS_SILENCE_FRAME {
            return Ok(opus_frame.to_vec());
        }
        self.encryptor
            .encrypt(opus_frame)
            .map_err(|failure| E2eeError::Encrypt(failure.describe().to_owned()))
    }

    fn decrypt(&mut self, sender: UserId, encrypted: &[u8]) -> Result<Vec<u8>, E2eeError> {
        if encrypted == frame::OPUS_SILENCE_FRAME {
            return Ok(encrypted.to_vec());
        }

        let now = Instant::now();
        let Some(decryptor) = self.decryptors.get_mut(&sender.get()) else {
            if self.receive_version() == 0 {
                return Ok(encrypted.to_vec());
            }
            return Err(E2eeError::Decrypt("the sender is not known".to_owned()));
        };
        decryptor
            .decrypt(encrypted, now)
            .map_err(|failure| E2eeError::Decrypt(failure.describe().to_owned()))
    }
}
