//! A local simulation of a DAVE call. A test helper plays the voice gateway.

use openmls::prelude::tls_codec::{Deserialize as _, Serialize as _, Size as _};
use openmls::prelude::*;
use openmls_basic_credential::SignatureKeyPair;
use openmls_rust_crypto::OpenMlsRustCrypto;
use serde_json::json;

use super::*;

const CHANNEL: ChannelId = ChannelId::new_const(1_234_567_890_123);
const ALICE: UserId = UserId::new_const(111_111_111_111_111_111);
const BOB: UserId = UserId::new_const(222_222_222_222_222_222);
const CAROL: UserId = UserId::new_const(333_333_333_333_333_333);
const DAVE: UserId = UserId::new_const(444_444_444_444_444_444);

const OPUS_FRAME: &[u8] = &[
    0x78, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A,
];

/// The voice gateway: the MLS delivery service and external sender.
struct Gateway {
    provider: OpenMlsRustCrypto,
    signer: SignatureKeyPair,
    group_id: GroupId,
    epoch: u64,
}

impl Gateway {
    fn new() -> Self {
        Self {
            provider: OpenMlsRustCrypto::default(),
            signer: SignatureKeyPair::new(SignatureScheme::ECDSA_SECP256R1_SHA256).unwrap(),
            group_id: GroupId::from_slice(&CHANNEL.get().to_be_bytes()),
            epoch: 0,
        }
    }

    /// The payload of opcode 25.
    fn external_sender(&self) -> Vec<u8> {
        let credential: Credential = BasicCredential::new(vec![0x00, 0x01, 0x01, 0x00]).into();
        ExternalSender::new(self.signer.public().into(), credential)
            .tls_serialize_detached()
            .unwrap()
    }

    /// The payload of opcode 27 with Add proposals for key packages (opcode 26 payloads).
    fn propose_adds(&self, key_packages: &[&[u8]]) -> Vec<u8> {
        let messages: Vec<MlsMessageOut> = key_packages
            .iter()
            .map(|bytes| {
                let key_package = KeyPackageIn::tls_deserialize_exact(*bytes)
                    .unwrap()
                    .validate(self.provider.crypto(), ProtocolVersion::Mls10)
                    .unwrap();
                ExternalProposal::new_add::<OpenMlsRustCrypto>(
                    key_package,
                    self.group_id.clone(),
                    GroupEpoch::from(self.epoch),
                    &self.signer,
                    SenderExtensionIndex::new(0),
                )
                .unwrap()
            })
            .collect();
        append_payload(&messages)
    }

    /// The payload of opcode 27 with a Remove proposal.
    fn propose_remove(&self, leaf_index: LeafNodeIndex) -> Vec<u8> {
        let message = ExternalProposal::new_remove::<OpenMlsRustCrypto>(
            leaf_index,
            self.group_id.clone(),
            GroupEpoch::from(self.epoch),
            &self.signer,
            SenderExtensionIndex::new(0),
        )
        .unwrap();
        append_payload(&[message])
    }
}

fn append_payload(messages: &[MlsMessageOut]) -> Vec<u8> {
    let mut payload = vec![0];
    payload.extend(messages.to_vec().tls_serialize_detached().unwrap());
    payload
}

/// Splits the payload of opcode 28 into the commit and the optional Welcome.
fn split_commit_welcome(payload: &[u8]) -> (Vec<u8>, Option<Vec<u8>>) {
    let mut reader = payload;
    let commit = MlsMessageIn::tls_deserialize(&mut reader).unwrap();
    let commit_size = commit.tls_serialized_len();
    let rest = &payload[commit_size..];
    if rest.is_empty() {
        return (payload.to_vec(), None);
    }
    Welcome::tls_deserialize_exact(rest).unwrap();
    (payload[..commit_size].to_vec(), Some(rest.to_vec()))
}

fn with_transition(transition_id: u16, body: &[u8]) -> Vec<u8> {
    let mut payload = transition_id.to_be_bytes().to_vec();
    payload.extend_from_slice(body);
    payload
}

fn binary(messages: &[E2eeOutgoing], expected_op: u8) -> Vec<u8> {
    let found: Vec<&Vec<u8>> = messages
        .iter()
        .filter_map(|message| match message {
            E2eeOutgoing::Binary { op, payload } if *op == expected_op => Some(payload),
            _ => None,
        })
        .collect();
    assert_eq!(
        found.len(),
        1,
        "one binary opcode {expected_op} in {messages:?}"
    );
    found[0].clone()
}

fn ready(transition_id: u16) -> E2eeOutgoing {
    E2eeOutgoing::Json {
        op: 23,
        data: json!({ "transition_id": transition_id }),
    }
}

fn execute(session: &mut DaveSession, transition_id: u16) {
    let output = session
        .json_message(22, &json!({ "transition_id": transition_id }))
        .unwrap();
    assert!(output.is_empty());
}

/// Makes a session that joined the call, and gives its key package.
fn join(gateway: &Gateway, user: UserId, others: &[UserId]) -> (DaveSession, Vec<u8>) {
    let mut session = DaveSession::new().unwrap();
    assert_eq!(session.max_protocol_version(), 1);
    session.clients_connect(others);
    let output = session.session_description(1, user, CHANNEL).unwrap();
    let key_package = binary(&output, 26);
    assert!(
        session
            .binary_message(25, &gateway.external_sender())
            .unwrap()
            .is_empty()
    );
    (session, key_package)
}

/// Builds a group of Alice, Bob, and Carol. Alice commits. Gives the sessions.
fn three_member_group(gateway: &mut Gateway) -> (DaveSession, DaveSession, DaveSession) {
    let (mut alice, _) = join(gateway, ALICE, &[BOB, CAROL]);
    let (mut bob, bob_key_package) = join(gateway, BOB, &[ALICE, CAROL]);
    let (mut carol, carol_key_package) = join(gateway, CAROL, &[ALICE, BOB]);

    let proposals = gateway.propose_adds(&[&bob_key_package, &carol_key_package]);
    let output = alice.binary_message(27, &proposals).unwrap();
    let (commit, welcome) = split_commit_welcome(&binary(&output, 28));
    let welcome = welcome.expect("a welcome for the added members");

    assert_eq!(
        alice
            .binary_message(29, &with_transition(1, &commit))
            .unwrap(),
        [ready(1)]
    );
    assert_eq!(
        bob.binary_message(30, &with_transition(1, &welcome))
            .unwrap(),
        [ready(1)]
    );
    assert_eq!(
        carol
            .binary_message(30, &with_transition(1, &welcome))
            .unwrap(),
        [ready(1)]
    );
    gateway.epoch = 1;

    for session in [&mut alice, &mut bob, &mut carol] {
        assert_eq!(session.mls.epoch(), Some(1));
        execute(session, 1);
    }
    (alice, bob, carol)
}

#[test]
fn full_call_encrypts_between_members_and_excludes_removed_member() {
    let mut gateway = Gateway::new();
    let (mut alice, mut bob, mut carol) = three_member_group(&mut gateway);

    // Each member decrypts the frames of each other member.
    let from_alice = alice.encrypt(OPUS_FRAME).unwrap();
    assert_ne!(from_alice, OPUS_FRAME);
    assert_eq!(bob.decrypt(ALICE, &from_alice).unwrap(), OPUS_FRAME);
    assert_eq!(carol.decrypt(ALICE, &from_alice).unwrap(), OPUS_FRAME);
    let from_bob = bob.encrypt(OPUS_FRAME).unwrap();
    assert_eq!(alice.decrypt(BOB, &from_bob).unwrap(), OPUS_FRAME);
    assert_eq!(carol.decrypt(BOB, &from_bob).unwrap(), OPUS_FRAME);
    let from_carol = carol.encrypt(OPUS_FRAME).unwrap();
    assert_eq!(alice.decrypt(CAROL, &from_carol).unwrap(), OPUS_FRAME);

    // A replay of a frame fails.
    assert!(bob.decrypt(ALICE, &from_alice).is_err());

    // Carol leaves. The gateway proposes her removal to Alice and Bob. Both commit, and the
    // gateway selects the commit of Alice.
    let carol_leaf = alice.mls.leaf_index_of(CAROL.get()).unwrap();
    alice.client_disconnect(CAROL);
    bob.client_disconnect(CAROL);
    let proposals = gateway.propose_remove(carol_leaf);
    let (alice_commit, alice_welcome) =
        split_commit_welcome(&binary(&alice.binary_message(27, &proposals).unwrap(), 28));
    let (bob_commit, _) =
        split_commit_welcome(&binary(&bob.binary_message(27, &proposals).unwrap(), 28));
    assert!(alice_welcome.is_none());
    assert_ne!(alice_commit, bob_commit);

    assert_eq!(
        alice
            .binary_message(29, &with_transition(2, &alice_commit))
            .unwrap(),
        [ready(2)]
    );
    assert_eq!(
        bob.binary_message(29, &with_transition(2, &alice_commit))
            .unwrap(),
        [ready(2)]
    );
    assert_eq!(alice.mls.epoch(), Some(2));
    assert_eq!(bob.mls.epoch(), Some(2));

    // If Carol gets the proposal and the commit, she sees her removal and leaves the group.
    // She cannot commit her own removal.
    assert!(carol.binary_message(27, &proposals).is_err());
    assert!(
        carol
            .binary_message(29, &with_transition(2, &alice_commit))
            .unwrap()
            .is_empty()
    );
    assert!(!carol.mls.is_established());

    // A repeated external sender has no effect on an established group.
    assert!(
        alice
            .binary_message(25, &gateway.external_sender())
            .unwrap()
            .is_empty()
    );
    assert!(alice.mls.is_established());

    // Before the execution, Alice still sends with the old key. Bob decrypts it.
    let before_execute = alice.encrypt(OPUS_FRAME).unwrap();
    assert_eq!(bob.decrypt(ALICE, &before_execute).unwrap(), OPUS_FRAME);

    execute(&mut alice, 2);
    execute(&mut bob, 2);

    let from_alice = alice.encrypt(OPUS_FRAME).unwrap();
    let from_bob = bob.encrypt(OPUS_FRAME).unwrap();
    assert_eq!(bob.decrypt(ALICE, &from_alice).unwrap(), OPUS_FRAME);
    assert_eq!(alice.decrypt(BOB, &from_bob).unwrap(), OPUS_FRAME);

    // Carol has the keys of epoch 1 only.
    assert!(matches!(
        carol.decrypt(ALICE, &from_alice),
        Err(E2eeError::Decrypt(_))
    ));
    assert!(matches!(
        carol.decrypt(BOB, &from_bob),
        Err(E2eeError::Decrypt(_))
    ));
}

#[test]
fn own_commit_and_foreign_commit_in_pending_groups() {
    // Two pending members both commit the same proposal. The gateway selects Bob's commit.
    // Alice gets an Add proposal for Bob, and Bob gets one for Alice. Thus each one makes a
    // group. Alice then gets Bob's Welcome.
    let gateway = Gateway::new();
    let (mut alice, alice_key_package) = join(&gateway, ALICE, &[BOB]);
    let (mut bob, bob_key_package) = join(&gateway, BOB, &[ALICE]);

    let output = alice
        .binary_message(27, &gateway.propose_adds(&[&bob_key_package]))
        .unwrap();
    let (alice_commit, _) = split_commit_welcome(&binary(&output, 28));
    let output = bob
        .binary_message(27, &gateway.propose_adds(&[&alice_key_package]))
        .unwrap();
    let (bob_commit, bob_welcome) = split_commit_welcome(&binary(&output, 28));

    assert_eq!(
        bob.binary_message(29, &with_transition(3, &bob_commit))
            .unwrap(),
        [ready(3)]
    );
    assert_eq!(
        alice
            .binary_message(30, &with_transition(3, &bob_welcome.unwrap()))
            .unwrap(),
        [ready(3)]
    );

    // Alice is now in the group of Bob. Her own stale commit is a commit without proposals
    // for the new group, so it is not valid.
    let output = alice
        .binary_message(29, &with_transition(4, &alice_commit))
        .unwrap();
    assert_eq!(output[0], invalid_commit_welcome(4));
    binary(&output, 26);

    // The receive keys of the last transition stay after the reset, as in libdave.
    execute(&mut bob, 3);
    let from_bob = bob.encrypt(OPUS_FRAME).unwrap();
    assert_eq!(alice.decrypt(BOB, &from_bob).unwrap(), OPUS_FRAME);
}

#[test]
fn foreign_commit_before_welcome_is_invalid() {
    let gateway = Gateway::new();
    let (mut alice, alice_key_package) = join(&gateway, ALICE, &[BOB]);
    let (mut bob, bob_key_package) = join(&gateway, BOB, &[ALICE]);

    alice
        .binary_message(27, &gateway.propose_adds(&[&bob_key_package]))
        .unwrap();
    let output = bob
        .binary_message(27, &gateway.propose_adds(&[&alice_key_package]))
        .unwrap();
    let (bob_commit, _) = split_commit_welcome(&binary(&output, 28));

    let output = alice
        .binary_message(29, &with_transition(5, &bob_commit))
        .unwrap();
    assert_eq!(output.len(), 2);
    assert_eq!(output[0], invalid_commit_welcome(5));
    binary(&output, 26);
}

#[test]
fn welcome_with_other_external_sender_is_invalid() {
    let mut gateway = Gateway::new();
    let (mut alice, _) = join(&gateway, ALICE, &[BOB]);
    let (mut bob, bob_key_package) = join(&gateway, BOB, &[ALICE]);

    // Bob got the external sender of another gateway.
    let other_gateway = Gateway::new();
    bob.binary_message(25, &other_gateway.external_sender())
        .unwrap();

    let output = alice
        .binary_message(27, &gateway.propose_adds(&[&bob_key_package]))
        .unwrap();
    let (commit, welcome) = split_commit_welcome(&binary(&output, 28));
    alice
        .binary_message(29, &with_transition(1, &commit))
        .unwrap();
    gateway.epoch = 1;

    let output = bob
        .binary_message(30, &with_transition(1, &welcome.unwrap()))
        .unwrap();
    assert_eq!(output.len(), 2);
    assert_eq!(output[0], invalid_commit_welcome(1));
    binary(&output, 26);
    assert!(!bob.mls.is_established());
}

#[test]
fn add_for_unknown_user_is_refused() {
    let gateway = Gateway::new();
    let (mut alice, _) = join(&gateway, ALICE, &[BOB]);
    let (_dave, dave_key_package) = join(&gateway, DAVE, &[ALICE]);

    let result = alice.binary_message(27, &gateway.propose_adds(&[&dave_key_package]));
    assert!(matches!(result, Err(E2eeError::Protocol(_))));

    // A user that disconnected is not acceptable either.
    let (mut alice, _) = join(&gateway, ALICE, &[DAVE]);
    alice.client_disconnect(DAVE);
    let result = alice.binary_message(27, &gateway.propose_adds(&[&dave_key_package]));
    assert!(matches!(result, Err(E2eeError::Protocol(_))));
}

#[test]
fn revoked_proposal_gives_no_commit() {
    let gateway = Gateway::new();
    let (mut alice, _) = join(&gateway, ALICE, &[BOB]);
    let (_bob, bob_key_package) = join(&gateway, BOB, &[ALICE]);

    let append = gateway.propose_adds(&[&bob_key_package]);
    binary(&alice.binary_message(27, &append).unwrap(), 28);

    // The proposal reference: RefHash("MLS 1.0 Proposal Reference", AuthenticatedContent).
    let pending = alice.mls.pending_proposal_refs();
    assert_eq!(pending.len(), 1);
    let mut revoke = vec![1];
    revoke.extend(pending.tls_serialize_detached().unwrap());
    assert!(alice.binary_message(27, &revoke).unwrap().is_empty());

    // An unknown reference is an error.
    assert!(alice.binary_message(27, &revoke).is_err());
}

#[test]
fn invalid_commit_resets_the_group() {
    let mut gateway = Gateway::new();
    let (mut alice, mut bob, _carol) = three_member_group(&mut gateway);

    // Proposals are pending, but the commit is garbage.
    let carol_leaf = alice.mls.leaf_index_of(CAROL.get()).unwrap();
    alice.client_disconnect(CAROL);
    alice
        .binary_message(27, &gateway.propose_remove(carol_leaf))
        .unwrap();
    let output = alice
        .binary_message(29, &with_transition(7, &[0xFF; 20]))
        .unwrap();
    assert_eq!(output[0], invalid_commit_welcome(7));
    binary(&output, 26);
    assert!(!alice.mls.is_established());

    // A commit without proposals is not valid for an established group.
    let output = bob
        .binary_message(29, &with_transition(8, &[0; 4]))
        .unwrap();
    assert_eq!(output[0], invalid_commit_welcome(8));
}

#[test]
fn version_zero_passes_through() {
    let mut session = DaveSession::new().unwrap();
    assert!(
        session
            .session_description(0, ALICE, CHANNEL)
            .unwrap()
            .is_empty()
    );
    assert_eq!(session.encrypt(OPUS_FRAME).unwrap(), OPUS_FRAME);
    assert_eq!(session.decrypt(BOB, OPUS_FRAME).unwrap(), OPUS_FRAME);
    session.clients_connect(&[BOB]);
    assert_eq!(session.decrypt(BOB, OPUS_FRAME).unwrap(), OPUS_FRAME);
}

#[test]
fn downgrade_to_version_zero() {
    let mut gateway = Gateway::new();
    let (mut alice, mut bob, _carol) = three_member_group(&mut gateway);

    let output = alice
        .json_message(21, &json!({ "protocol_version": 0, "transition_id": 9 }))
        .unwrap();
    assert_eq!(output, [ready(9)]);
    bob.json_message(21, &json!({ "protocol_version": 0, "transition_id": 9 }))
        .unwrap();

    // Bob already accepts plaintext, and still decrypts frames in flight.
    let in_flight = alice.encrypt(OPUS_FRAME).unwrap();
    assert_eq!(bob.decrypt(ALICE, OPUS_FRAME).unwrap(), OPUS_FRAME);
    assert_eq!(bob.decrypt(ALICE, &in_flight).unwrap(), OPUS_FRAME);

    execute(&mut alice, 9);
    assert_eq!(alice.encrypt(OPUS_FRAME).unwrap(), OPUS_FRAME);
    assert!(!alice.mls.is_established());
}

#[test]
fn prepare_epoch_one_makes_new_key_package() {
    let gateway = Gateway::new();
    let (mut alice, _) = join(&gateway, ALICE, &[]);
    let output = alice
        .json_message(24, &json!({ "protocol_version": 1, "epoch": 1 }))
        .unwrap();
    binary(&output, 26);
    let output = alice
        .json_message(24, &json!({ "protocol_version": 1, "epoch": "1" }))
        .unwrap();
    binary(&output, 26);
    assert!(
        alice
            .json_message(24, &json!({ "protocol_version": 1, "epoch": 5 }))
            .unwrap()
            .is_empty()
    );

    // The sole member reset: transition 0 executes at once and sends nothing.
    assert!(
        alice
            .json_message(21, &json!({ "protocol_version": 1, "transition_id": 0 }))
            .unwrap()
            .is_empty()
    );
    // No group is established, so the session has no send key.
    assert!(matches!(
        alice.encrypt(OPUS_FRAME),
        Err(E2eeError::Encrypt(_))
    ));
}

#[test]
fn silence_frame_passes_both_ways() {
    let mut gateway = Gateway::new();
    let (mut alice, mut bob, _carol) = three_member_group(&mut gateway);
    let silence = [0xF8, 0xFF, 0xFE];
    assert_eq!(alice.encrypt(&silence).unwrap(), silence);
    assert_eq!(bob.decrypt(ALICE, &silence).unwrap(), silence);
    assert_eq!(bob.decrypt(DAVE, &silence).unwrap(), silence);
}

#[test]
fn key_package_has_required_parameters() {
    let gateway = Gateway::new();
    let (_alice, key_package) = join(&gateway, ALICE, &[]);
    let key_package = KeyPackageIn::tls_deserialize_exact(key_package.as_slice())
        .unwrap()
        .validate(gateway.provider.crypto(), ProtocolVersion::Mls10)
        .unwrap();

    assert_eq!(
        key_package.ciphersuite(),
        Ciphersuite::MLS_128_DHKEMP256_AES128GCM_SHA256_P256
    );
    assert_eq!(key_package.life_time().not_before(), 0);
    assert_eq!(key_package.life_time().not_after(), u64::MAX);
    let leaf = key_package.leaf_node();
    assert_eq!(leaf.credential().credential_type(), CredentialType::Basic);
    assert_eq!(
        leaf.credential().serialized_content(),
        ALICE.get().to_be_bytes()
    );
    let capabilities = leaf.capabilities();
    assert!(
        capabilities
            .ciphersuites()
            .iter()
            .all(|suite| !suite.is_grease())
    );
    assert!(
        capabilities
            .credentials()
            .iter()
            .all(|credential| !credential.is_grease())
    );
    assert!(capabilities.extensions().is_empty());
    assert!(leaf.extensions().iter().next().is_none());
}

#[test]
fn malformed_messages_give_errors() {
    let gateway = Gateway::new();
    let (mut alice, _) = join(&gateway, ALICE, &[BOB]);

    assert!(alice.binary_message(99, &[]).is_err());
    assert!(alice.json_message(99, &json!({})).is_err());
    assert!(
        alice
            .json_message(21, &json!({ "transition_id": 1 }))
            .is_err()
    );
    assert!(
        alice
            .json_message(22, &json!({ "transition_id": "x" }))
            .is_err()
    );
    assert!(
        alice
            .json_message(22, &json!({ "transition_id": 70_000 }))
            .is_err()
    );

    let samples: [&[u8]; 6] = [&[], &[0], &[1], &[2, 0], &[0, 0x40], &[0xFF; 64]];
    for sample in samples {
        let _ = alice.binary_message(25, sample);
        let _ = alice.binary_message(27, sample);
        let _ = alice.binary_message(29, sample);
        let _ = alice.binary_message(30, sample);
    }
    assert!(alice.binary_message(25, &[0xFF; 64]).is_err());
    assert!(alice.binary_message(29, &[0]).is_err());
    assert!(alice.binary_message(30, &[0]).is_err());

    // The session still works after the malformed input.
    alice
        .binary_message(25, &gateway.external_sender())
        .unwrap();
}
