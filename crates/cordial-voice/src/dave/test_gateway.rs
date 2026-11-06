//! A test stand-in for the DAVE part of the voice gateway: the MLS external sender.
//!
//! The unit tests and the libdave interoperability tests (`interop/libdave`) use it. It is not
//! part of the public API and can change in any release.

use std::fmt;

use openmls::prelude::tls_codec::{Deserialize as _, Serialize as _, Size as _};
use openmls::prelude::*;

use super::provider::{DaveProvider, SignatureKeyPair};

/// The external sender of one call. It signs the proposals of opcode 27.
pub struct TestGateway {
    pub(crate) provider: DaveProvider,
    signer: SignatureKeyPair,
    group_id: GroupId,
    /// The MLS epoch of the group. Set it after each commit, because a proposal names its epoch.
    pub epoch: u64,
}

impl fmt::Debug for TestGateway {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestGateway")
            .field("group_id", &self.group_id)
            .field("epoch", &self.epoch)
            .finish_non_exhaustive()
    }
}

impl TestGateway {
    /// Makes the external sender for the call in a channel. The epoch starts at 0.
    ///
    /// # Panics
    ///
    /// Panics when the signature key pair cannot be made.
    #[must_use]
    pub fn new(channel_id: u64) -> Self {
        Self {
            provider: DaveProvider::default(),
            signer: SignatureKeyPair::generate().expect("a signature key pair"),
            group_id: GroupId::from_slice(&channel_id.to_be_bytes()),
            epoch: 0,
        }
    }

    /// Gives the payload of opcode 25.
    ///
    /// # Panics
    ///
    /// Panics when the encoding fails.
    #[must_use]
    pub fn external_sender(&self) -> Vec<u8> {
        let credential: Credential = BasicCredential::new(vec![0x00, 0x01, 0x01, 0x00]).into();
        ExternalSender::new(self.signer.public().into(), credential)
            .tls_serialize_detached()
            .expect("an encoded external sender")
    }

    /// Gives the payload of opcode 27 with Add proposals for key packages (opcode 26 payloads).
    ///
    /// # Panics
    ///
    /// Panics when a key package is not valid.
    #[must_use]
    pub fn propose_adds(&self, key_packages: &[&[u8]]) -> Vec<u8> {
        let messages: Vec<MlsMessageOut> = key_packages
            .iter()
            .map(|bytes| {
                let key_package = KeyPackageIn::tls_deserialize_exact(*bytes)
                    .expect("a decoded key package")
                    .validate(self.provider.crypto(), ProtocolVersion::Mls10)
                    .expect("a valid key package");
                ExternalProposal::new_add::<DaveProvider>(
                    key_package,
                    self.group_id.clone(),
                    GroupEpoch::from(self.epoch),
                    &self.signer,
                    SenderExtensionIndex::new(0),
                )
                .expect("an Add proposal")
            })
            .collect();
        append_payload(&messages)
    }

    /// Gives the payload of opcode 27 with a Remove proposal for a leaf index.
    ///
    /// # Panics
    ///
    /// Panics when the proposal cannot be made.
    #[must_use]
    pub fn propose_remove(&self, leaf_index: u32) -> Vec<u8> {
        let message = ExternalProposal::new_remove::<DaveProvider>(
            LeafNodeIndex::new(leaf_index),
            self.group_id.clone(),
            GroupEpoch::from(self.epoch),
            &self.signer,
            SenderExtensionIndex::new(0),
        )
        .expect("a Remove proposal");
        append_payload(&[message])
    }
}

/// Gives an opcode 27 payload that appends proposals: the operation byte 0, then the messages.
fn append_payload(messages: &[MlsMessageOut]) -> Vec<u8> {
    let mut payload = vec![0];
    payload.extend(
        messages
            .to_vec()
            .tls_serialize_detached()
            .expect("encoded proposals"),
    );
    payload
}

/// Splits the payload of opcode 28 into the commit and the optional Welcome.
///
/// # Panics
///
/// Panics when the payload is not a commit and an optional Welcome.
#[must_use]
pub fn split_commit_welcome(payload: &[u8]) -> (Vec<u8>, Option<Vec<u8>>) {
    let mut reader = payload;
    let commit = MlsMessageIn::tls_deserialize(&mut reader).expect("a commit message");
    let commit_size = commit.tls_serialized_len();
    let rest = &payload[commit_size..];
    if rest.is_empty() {
        return (payload.to_vec(), None);
    }

    Welcome::tls_deserialize_exact(rest).expect("a Welcome after the commit");
    (payload[..commit_size].to_vec(), Some(rest.to_vec()))
}

/// Puts the `u16` transition ID (big-endian) in front of a commit or a Welcome (opcodes 29, 30).
#[must_use]
pub fn with_transition(transition_id: u16, body: &[u8]) -> Vec<u8> {
    let mut payload = transition_id.to_be_bytes().to_vec();
    payload.extend_from_slice(body);
    payload
}
