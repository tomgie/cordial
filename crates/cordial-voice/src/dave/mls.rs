//! The MLS group of a DAVE session, on OpenMLS.
//!
//! The logic follows libdave `mls/session.cpp`. The session has these states:
//!
//! - A pending group: a local group with the own user as the only member, at epoch 0. The
//!   session makes it when it has the group ID and the external sender.
//! - An established group: the group after the own commit or a Welcome.
//!
//! Each group has its own OpenMLS provider (storage), because the pending group and the group
//! from a Welcome have the same group ID. The key package for a Welcome is in a separate
//! provider. That provider becomes the provider of the group after the Welcome.

use std::collections::HashSet;
use std::fmt;

use openmls::ciphersuite::hash_ref::ProposalRef;
use openmls::prelude::tls_codec::{Deserialize as _, Serialize as _};
use openmls::prelude::*;

use super::provider::{CIPHERSUITE, DaveProvider, SignatureKeyPair};
use super::ratchet::KeyRatchet;

/// The exporter label of the sender base secret (libdave `mls/session.h`,
/// `USER_MEDIA_KEY_BASE_LABEL`).
const SENDER_KEY_LABEL: &str = "Discord Secure Frames v0";

/// The size of the sender base secret (libdave `kAesGcm128KeyBytes`).
const SENDER_KEY_SIZE: usize = 16;

/// The proposals operation of opcode 27.
const OPERATION_APPEND: u8 = 0;
const OPERATION_REVOKE: u8 = 1;

/// An error of the MLS layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MlsError(pub(crate) String);

impl fmt::Display for MlsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn error(message: impl Into<String>) -> MlsError {
    MlsError(message.into())
}

fn fail(context: &str, cause: impl fmt::Display) -> MlsError {
    MlsError(format!("{context}: {cause}"))
}

/// The result of an announced commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommitOutcome {
    /// The group moved to the new epoch.
    Applied,
    /// The session has no group state, so it ignores the commit (libdave `ignored_t`).
    Ignored,
    /// The commit removed the own user from the group.
    Removed,
}

/// The leaf node capabilities: MLS 1.0, ciphersuite 2, and basic credentials only. No GREASE
/// values (libdave `LeafNodeCapabilitiesForProtocolVersion`, mlspp built with
/// `DISABLE_GREASE`).
fn capabilities() -> Capabilities {
    Capabilities::builder()
        .versions(vec![ProtocolVersion::Mls10])
        .ciphersuites(vec![CIPHERSUITE])
        .extensions(Vec::new())
        .proposals(Vec::new())
        .credentials(vec![CredentialType::Basic])
        .build()
}

/// The lifetime that the voice gateway requires: `not_before = 0` and `not_after = 2^64 - 1`.
fn lifetime() -> Lifetime {
    Lifetime::init(0, u64::MAX)
}

/// The basic credential of a user: the user ID as a big-endian `u64` (libdave
/// `mls/user_credential.cpp`).
fn user_credential(user_id: u64) -> Credential {
    BasicCredential::new(user_id.to_be_bytes().to_vec()).into()
}

/// Gives the user ID of a basic credential.
fn credential_user_id(credential: &Credential) -> Option<u64> {
    if credential.credential_type() != CredentialType::Basic {
        return None;
    }
    let identity: [u8; 8] = credential.serialized_content().try_into().ok()?;
    Some(u64::from_be_bytes(identity))
}

fn join_config() -> MlsGroupJoinConfig {
    MlsGroupJoinConfig::builder()
        .wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
        .use_ratchet_tree_extension(true)
        .build()
}

/// Checks that each identity in the roster occurs one time.
fn check_unique_members(members: impl Iterator<Item = Member>) -> Result<(), MlsError> {
    let mut seen = HashSet::new();
    for member in members {
        let user_id = credential_user_id(&member.credential)
            .ok_or_else(|| error("a member has no user ID credential"))?;
        if !seen.insert(user_id) {
            return Err(error("the roster has a duplicate user ID"));
        }
    }
    Ok(())
}

/// An MLS group and its provider.
struct Group {
    provider: DaveProvider,
    group: MlsGroup,
}

/// The MLS state of a DAVE session.
pub(crate) struct MlsSession {
    own_user_id: u64,
    group_id: Vec<u8>,
    signer: SignatureKeyPair,
    external_sender: Option<ExternalSender>,
    /// The provider that holds the current key package.
    join: Option<DaveProvider>,
    pending: Option<Group>,
    established: Option<Group>,
    /// Whether an opcode 27 arrived for the current epoch (libdave `stateWithProposals_`).
    has_proposals: bool,
    /// The serialized commit that the own user made for the current epoch.
    own_commit: Option<Vec<u8>>,
}

impl fmt::Debug for MlsSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MlsSession")
            .field("own_user_id", &self.own_user_id)
            .field("group_id", &self.group_id)
            .field("has_external_sender", &self.external_sender.is_some())
            .field("has_pending_group", &self.pending.is_some())
            .field(
                "epoch",
                &self
                    .established
                    .as_ref()
                    .map(|group| group.group.epoch().as_u64()),
            )
            .finish_non_exhaustive()
    }
}

impl MlsSession {
    /// Makes an MLS session with a new signature key pair.
    pub(crate) fn new() -> Result<Self, MlsError> {
        let signer = SignatureKeyPair::generate()
            .map_err(|cause| fail("cannot make the signature key pair", format!("{cause:?}")))?;
        Ok(Self {
            own_user_id: 0,
            group_id: Vec::new(),
            signer,
            external_sender: None,
            join: None,
            pending: None,
            established: None,
            has_proposals: false,
            own_commit: None,
        })
    }

    /// Resets the group state and prepares a group for a user and a group ID. The external
    /// sender stays (libdave `Session::Init`).
    pub(crate) fn init(&mut self, own_user_id: u64, group_id: u64) -> Result<(), MlsError> {
        self.reset();
        self.own_user_id = own_user_id;
        self.group_id = group_id.to_be_bytes().to_vec();
        if self.external_sender.is_some() {
            self.create_pending_group()?;
        }
        Ok(())
    }

    /// Removes all group state (libdave `Session::Reset`).
    pub(crate) fn reset(&mut self) {
        self.clear_pending_state();
        self.established = None;
        self.group_id.clear();
    }

    fn clear_pending_state(&mut self) {
        self.pending = None;
        self.join = None;
        self.has_proposals = false;
        self.own_commit = None;
    }

    /// Gives whether the session has an established group.
    pub(crate) fn is_established(&self) -> bool {
        self.established.is_some()
    }

    fn credential_with_key(&self) -> CredentialWithKey {
        CredentialWithKey {
            credential: user_credential(self.own_user_id),
            signature_key: self.signer.public().into(),
        }
    }

    /// Sets the external sender of opcode 25 (libdave `Session::SetExternalSender`).
    ///
    /// The group of an established session keeps its external sender. libdave ignores a new
    /// external sender in this state, and so does this function.
    pub(crate) fn set_external_sender(&mut self, payload: &[u8]) -> Result<(), MlsError> {
        let sender = ExternalSender::tls_deserialize_exact(payload)
            .map_err(|cause| fail("malformed external sender", cause))?;
        if self.established.is_some() {
            if self.external_sender.as_ref() != Some(&sender) {
                tracing::warn!("DAVE external sender changed after the group was established");
            }
            return Ok(());
        }
        self.external_sender = Some(sender);

        if !self.group_id.is_empty() {
            self.create_pending_group()?;
        }
        Ok(())
    }

    /// Makes the local group with the own user as the only member (libdave
    /// `Session::CreatePendingGroup`).
    fn create_pending_group(&mut self) -> Result<(), MlsError> {
        let sender = self
            .external_sender
            .clone()
            .ok_or_else(|| error("no external sender"))?;
        let extensions = Extensions::single(Extension::ExternalSenders(vec![sender]))
            .map_err(|cause| fail("cannot make the group extensions", cause))?;

        let provider = DaveProvider::default();
        let group = MlsGroup::builder()
            .with_group_id(GroupId::from_slice(&self.group_id))
            .ciphersuite(CIPHERSUITE)
            .with_wire_format_policy(PURE_PLAINTEXT_WIRE_FORMAT_POLICY)
            .use_ratchet_tree_extension(true)
            .with_capabilities(capabilities())
            .lifetime(lifetime())
            .with_group_context_extensions(extensions)
            .build(&provider, &self.signer, self.credential_with_key())
            .map_err(|cause| fail("cannot make the pending group", cause))?;

        self.pending = Some(Group { provider, group });
        Ok(())
    }

    /// Makes a new key package and gives it in TLS encoding (libdave
    /// `Session::GetMarshalledKeyPackage`).
    ///
    /// libdave sends the bare `KeyPackage` structure (`tls::marshal(*joinKeyPackage_)`), not an
    /// `MLSMessage`. This function does the same.
    pub(crate) fn key_package(&mut self) -> Result<Vec<u8>, MlsError> {
        let provider = DaveProvider::default();
        let bundle = KeyPackage::builder()
            .key_package_lifetime(lifetime())
            .leaf_node_capabilities(capabilities())
            .build(
                CIPHERSUITE,
                &provider,
                &self.signer,
                self.credential_with_key(),
            )
            .map_err(|cause| fail("cannot make the key package", cause))?;
        let bytes = bundle
            .key_package()
            .tls_serialize_detached()
            .map_err(|cause| fail("cannot encode the key package", cause))?;

        self.join = Some(provider);
        Ok(bytes)
    }

    /// Handles opcode 27. Gives the payload of opcode 28 when proposals are pending.
    ///
    /// `recognized` holds the users that can be in the call, with the own user.
    pub(crate) fn process_proposals(
        &mut self,
        payload: &[u8],
        recognized: &HashSet<u64>,
    ) -> Result<Option<Vec<u8>>, MlsError> {
        let Some(Group { provider, group }) = self.established.as_mut().or(self.pending.as_mut())
        else {
            // Without a local group, the proposals have no use.
            return Ok(None);
        };

        let (&operation, body) = payload
            .split_first()
            .ok_or_else(|| error("empty proposals message"))?;

        // A new set of proposals makes a new commit.
        group
            .clear_pending_commit(provider.storage())
            .map_err(|cause| fail("cannot clear the pending commit", format!("{cause:?}")))?;
        self.own_commit = None;
        self.has_proposals = true;

        match operation {
            OPERATION_APPEND => {
                let messages = Vec::<MlsMessageIn>::tls_deserialize_exact(body)
                    .map_err(|cause| fail("malformed proposal messages", cause))?;
                for message in messages {
                    append_proposal(provider, group, message, recognized)?;
                }
            }
            OPERATION_REVOKE => {
                let references = Vec::<ProposalRef>::tls_deserialize_exact(body)
                    .map_err(|cause| fail("malformed proposal references", cause))?;
                for reference in &references {
                    group
                        .remove_pending_proposal(provider.storage(), reference)
                        .map_err(|cause| fail("cannot revoke the proposal", cause))?;
                }
            }
            other => return Err(error(format!("unknown proposals operation {other}"))),
        }

        if group.pending_proposals().next().is_none() {
            return Ok(None);
        }

        let bundle = group
            .commit_builder()
            .consume_proposal_store(true)
            .load_psks(provider.storage())
            .map_err(|cause| fail("cannot prepare the commit", cause))?
            .build(provider.rand(), provider.crypto(), &self.signer, |_| true)
            .map_err(|cause| fail("cannot build the commit", cause))?
            .stage_commit(provider)
            .map_err(|cause| fail("cannot stage the commit", cause))?;

        let commit = bundle
            .commit()
            .tls_serialize_detached()
            .map_err(|cause| fail("cannot encode the commit", cause))?;
        let mut output = commit.clone();
        if let Some(welcome) = bundle.welcome() {
            // Opcode 28 carries the bare `Welcome` structure after the commit message
            // (whitepaper 1.1.2 changelog, libdave `Session::ProcessProposals`).
            let welcome = welcome
                .tls_serialize_detached()
                .map_err(|cause| fail("cannot encode the welcome", cause))?;
            output.extend_from_slice(&welcome);
        }

        self.own_commit = Some(commit);
        Ok(Some(output))
    }

    /// Handles the commit of opcode 29 (libdave `Session::ProcessCommit`).
    pub(crate) fn process_commit(&mut self, commit: &[u8]) -> Result<CommitOutcome, MlsError> {
        if !self.has_proposals {
            if self.established.is_none() {
                // A commit can arrive during a reset. This is not a failure.
                return Ok(CommitOutcome::Ignored);
            }
            return Err(error("commit without queued proposals"));
        }

        let is_own_commit = self.own_commit.as_deref() == Some(commit);
        if self.established.is_none() && !is_own_commit {
            return Err(error("unexpected commit before the welcome"));
        }

        let message = MlsMessageIn::tls_deserialize_exact(commit)
            .map_err(|cause| fail("malformed commit", cause))?;
        let message = message
            .try_into_protocol_message()
            .map_err(|cause| fail("the commit is not a protocol message", cause))?;

        let from_pending = self.established.is_none();
        let Some(Group { provider, group }) = self.established.as_mut().or(self.pending.as_mut())
        else {
            return Err(error("no group for the commit"));
        };
        check_handshake(&message, group, &self.group_id, ContentType::Commit)?;

        if is_own_commit {
            group
                .merge_pending_commit(provider)
                .map_err(|cause| fail("cannot merge the own commit", cause))?;
        } else {
            group
                .clear_pending_commit(provider.storage())
                .map_err(|cause| fail("cannot clear the pending commit", format!("{cause:?}")))?;
            let processed = group
                .process_message(provider, message)
                .map_err(|cause| fail("cannot process the commit", cause))?;
            if !matches!(processed.sender(), Sender::Member(_)) {
                return Err(error("the commit sender is not a member"));
            }
            let ProcessedMessageContent::StagedCommitMessage(staged) = processed.into_content()
            else {
                return Err(error("the message is not a commit"));
            };
            let all_references = staged
                .queued_proposals()
                .all(|proposal| proposal.proposal_or_ref_type() == ProposalOrRefType::Reference);
            if !all_references {
                return Err(error("the commit has a proposal that is not a reference"));
            }
            group
                .merge_staged_commit(provider, *staged)
                .map_err(|cause| fail("cannot merge the commit", cause))?;
        }

        if !group.is_active() {
            // The voice gateway removed the own user. Keep the key package, because a Welcome
            // for it can follow.
            self.established = None;
            self.pending = None;
            self.has_proposals = false;
            self.own_commit = None;
            if self.external_sender.is_some() {
                self.create_pending_group()?;
            }
            return Ok(CommitOutcome::Removed);
        }

        check_unique_members(group.members())?;

        if from_pending {
            self.established = self.pending.take();
        }
        self.clear_pending_state();
        Ok(CommitOutcome::Applied)
    }

    /// Handles the Welcome of opcode 30 (libdave `Session::ProcessWelcome`).
    pub(crate) fn process_welcome(
        &mut self,
        welcome: &[u8],
        recognized: &HashSet<u64>,
    ) -> Result<(), MlsError> {
        if self.established.is_some() {
            return Err(error("the group is already established"));
        }
        let expected_sender = self
            .external_sender
            .clone()
            .ok_or_else(|| error("no external sender"))?;
        let provider = self
            .join
            .take()
            .ok_or_else(|| error("no key package for the welcome"))?;

        let welcome = Welcome::tls_deserialize_exact(welcome)
            .map_err(|cause| fail("malformed welcome", cause))?;
        let staged = StagedWelcome::new_from_welcome(&provider, &join_config(), welcome, None)
            .map_err(|cause| fail("cannot process the welcome", cause))?;

        let context = staged.group_context();
        if context.group_id().as_slice() != self.group_id {
            return Err(error("the welcome is for another group"));
        }
        if context.ciphersuite() != CIPHERSUITE {
            return Err(error("the welcome uses another ciphersuite"));
        }
        let senders = context
            .extensions()
            .external_senders()
            .ok_or_else(|| error("the welcome has no external senders extension"))?;
        if senders.len() != 1 {
            return Err(error("the welcome must have exactly one external sender"));
        }
        if senders[0] != expected_sender {
            return Err(error("the welcome has an unexpected external sender"));
        }
        for member in staged.members() {
            let user_id = credential_user_id(&member.credential)
                .ok_or_else(|| error("a member has no user ID credential"))?;
            if !recognized.contains(&user_id) {
                return Err(error(format!(
                    "the welcome has the unexpected user {user_id}"
                )));
            }
        }

        let group = staged
            .into_group(&provider)
            .map_err(|cause| fail("cannot join the group", cause))?;
        check_unique_members(group.members())?;

        self.clear_pending_state();
        self.established = Some(Group { provider, group });
        Ok(())
    }

    /// Gives the key ratchet of a sender in the current epoch. Gives `None` when no group is
    /// established (libdave `Session::GetKeyRatchet`).
    pub(crate) fn key_ratchet(&self, user_id: u64) -> Option<KeyRatchet> {
        let Group { provider, group } = self.established.as_ref()?;
        let base_secret = group
            .export_secret(
                provider.crypto(),
                SENDER_KEY_LABEL,
                &user_id.to_le_bytes(),
                SENDER_KEY_SIZE,
            )
            .ok()?;
        Some(KeyRatchet::new(base_secret))
    }

    /// Gives the epoch of the established group.
    #[cfg(test)]
    pub(crate) fn epoch(&self) -> Option<u64> {
        self.established
            .as_ref()
            .map(|group| group.group.epoch().as_u64())
    }

    /// Gives the references of the pending proposals of the local group.
    #[cfg(test)]
    pub(crate) fn pending_proposal_refs(&self) -> Vec<ProposalRef> {
        let Some(Group { group, .. }) = self.established.as_ref().or(self.pending.as_ref()) else {
            return Vec::new();
        };
        group
            .pending_proposals()
            .map(|proposal| proposal.proposal_reference_ref().clone())
            .collect()
    }

    /// Gives the leaf index of a user in the established group.
    #[cfg(test)]
    pub(crate) fn leaf_index_of(&self, user_id: u64) -> Option<LeafNodeIndex> {
        let group = &self.established.as_ref()?.group;
        group
            .members()
            .find(|member| credential_user_id(&member.credential) == Some(user_id))
            .map(|member| member.index)
    }
}

/// Checks the framing of a handshake message from the voice gateway.
fn check_handshake(
    message: &ProtocolMessage,
    group: &MlsGroup,
    group_id: &[u8],
    content_type: ContentType,
) -> Result<(), MlsError> {
    if !matches!(message, ProtocolMessage::PublicMessage(_)) {
        return Err(error("a handshake message must be a PublicMessage"));
    }
    if message.group_id().as_slice() != group_id {
        return Err(error("the message is for another group"));
    }
    if message.epoch() != group.epoch() {
        return Err(error("the message is for another epoch"));
    }
    if message.content_type() != content_type {
        return Err(error("the message has an unexpected content type"));
    }
    Ok(())
}

/// Checks and stores one proposal of opcode 27 (libdave `Session::ValidateProposalMessage`).
fn append_proposal(
    provider: &DaveProvider,
    group: &mut MlsGroup,
    message: MlsMessageIn,
    recognized: &HashSet<u64>,
) -> Result<(), MlsError> {
    let message = message
        .try_into_protocol_message()
        .map_err(|cause| fail("the proposal is not a protocol message", cause))?;
    let group_id = group.group_id().as_slice().to_vec();
    check_handshake(&message, group, &group_id, ContentType::Proposal)?;

    let processed = group
        .process_message(provider, message)
        .map_err(|cause| fail("cannot process the proposal", cause))?;
    if !matches!(processed.sender(), Sender::External(_)) {
        return Err(error("the proposal is not from the external sender"));
    }
    let ProcessedMessageContent::ProposalMessage(proposal) = processed.into_content() else {
        return Err(error("the message is not a proposal"));
    };

    match proposal.proposal() {
        Proposal::Add(add) => {
            let user_id = credential_user_id(add.key_package().leaf_node().credential())
                .ok_or_else(|| error("the added member has no user ID credential"))?;
            if !recognized.contains(&user_id) {
                return Err(error(format!(
                    "the add proposal is for the unexpected user {user_id}"
                )));
            }
        }
        Proposal::Remove(_) => {}
        _ => return Err(error("only add and remove proposals are allowed")),
    }

    group
        .store_pending_proposal(provider.storage(), *proposal)
        .map_err(|cause| fail("cannot store the proposal", format!("{cause:?}")))
}
