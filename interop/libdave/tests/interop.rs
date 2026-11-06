//! Interoperability tests of the cordial DAVE session against libdave.
//!
//! Each test makes a call with cordial members and libdave members. The test gateway of
//! cordial-voice plays the external sender. The tests need the libdave shared library, so they
//! are ignored by default. See `interop/libdave/README.md`.

use std::collections::HashMap;

use cordial_model::id::{ChannelId, UserId};
use cordial_voice::DaveSession;
use cordial_voice::dave::test_gateway::{TestGateway, split_commit_welcome, with_transition};
use cordial_voice::e2ee::{E2eeError, E2eeOutgoing, E2eeSession};
use libdave_interop::{self as libdave, CommitResult};
use serde_json::json;

const CHANNEL: u64 = 1_234_567_890_123;
const ALICE: u64 = 111_111_111_111_111_111;
const BOB: u64 = 222_222_222_222_222_222;
const CAROL: u64 = 333_333_333_333_333_333;

/// The Opus frame of silence. Both libraries send it without encryption.
const OPUS_SILENCE: [u8; 3] = [0xF8, 0xFF, 0xFE];

/// The number of frames that each direction sends in a test.
const FRAME_COUNT: usize = 200;

/// Gives Opus-like frames with different sizes and content. No frame is the silence frame.
fn opus_frames(count: usize) -> Vec<Vec<u8>> {
    (0..count)
        .map(|index| {
            let size = 3 + (index * 37) % 400;
            (0..size)
                .map(|position| u8::try_from((index * 31 + position * 7) % 251).unwrap())
                .collect()
        })
        .collect()
}

fn user(user_id: u64) -> UserId {
    UserId::new_const(user_id)
}

fn ready(transition_id: u16) -> E2eeOutgoing {
    E2eeOutgoing::Json {
        op: 23,
        data: json!({ "transition_id": transition_id }),
    }
}

/// Gives the payload of the one binary message with an opcode.
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

/// A call member that uses the cordial DAVE session.
struct CordialMember {
    session: DaveSession,
}

impl CordialMember {
    /// Joins the call and gives the key package (opcode 26).
    fn join(gateway: &TestGateway, user_id: u64, others: &[u64]) -> (Self, Vec<u8>) {
        let mut session = DaveSession::new().unwrap();
        let others: Vec<UserId> = others.iter().copied().map(user).collect();
        session.clients_connect(&others);
        let output = session
            .session_description(1, user(user_id), ChannelId::new_const(CHANNEL))
            .unwrap();
        let key_package = binary(&output, 26);

        let output = session
            .binary_message(25, &gateway.external_sender())
            .unwrap();
        assert!(output.is_empty());
        (Self { session }, key_package)
    }

    /// Handles opcode 27 and gives the commit and Welcome of opcode 28.
    fn proposals(&mut self, payload: &[u8]) -> (Vec<u8>, Option<Vec<u8>>) {
        let output = self.session.binary_message(27, payload).unwrap();
        split_commit_welcome(&binary(&output, 28))
    }

    fn commit(&mut self, transition_id: u16, commit: &[u8]) -> Vec<E2eeOutgoing> {
        self.session
            .binary_message(29, &with_transition(transition_id, commit))
            .unwrap()
    }

    fn welcome(&mut self, transition_id: u16, welcome: &[u8]) -> Vec<E2eeOutgoing> {
        self.session
            .binary_message(30, &with_transition(transition_id, welcome))
            .unwrap()
    }

    fn execute(&mut self, transition_id: u16) {
        let output = self
            .session
            .json_message(22, &json!({ "transition_id": transition_id }))
            .unwrap();
        assert!(output.is_empty());
    }

    fn encrypt(&mut self, frame: &[u8]) -> Vec<u8> {
        self.session.encrypt(frame).unwrap()
    }

    fn decrypt(&mut self, sender: u64, frame: &[u8]) -> Result<Vec<u8>, E2eeError> {
        self.session.decrypt(user(sender), frame)
    }
}

/// A call member that uses libdave.
struct LibdaveMember {
    user_id: u64,
    session: libdave::Session,
    encryptor: libdave::Encryptor,
    decryptors: HashMap<u64, libdave::Decryptor>,
}

impl LibdaveMember {
    /// Joins the call and gives the key package (opcode 26).
    fn join(gateway: &TestGateway, user_id: u64) -> (Self, Vec<u8>) {
        let session = libdave::Session::new(user_id, CHANNEL);
        session.set_external_sender(&gateway.external_sender());
        let key_package = session.key_package();
        assert!(!key_package.is_empty(), "libdave gave no key package");

        let member = Self {
            user_id,
            session,
            encryptor: libdave::Encryptor::new(),
            decryptors: HashMap::new(),
        };
        (member, key_package)
    }

    /// Uses the key ratchets of the current epoch, as at an executed transition.
    fn use_epoch(&mut self, others: &[u64]) {
        let own = self
            .session
            .key_ratchet(self.user_id)
            .expect("libdave has a key ratchet for the own user");
        self.encryptor.set_key_ratchet(&own);

        for &other in others {
            let ratchet = self
                .session
                .key_ratchet(other)
                .unwrap_or_else(|| panic!("libdave has a key ratchet for user {other}"));
            self.decryptors
                .entry(other)
                .or_default()
                .set_key_ratchet(&ratchet);
        }
    }

    fn encrypt(&mut self, frame: &[u8]) -> Vec<u8> {
        self.encryptor
            .encrypt(frame)
            .unwrap_or_else(|code| panic!("libdave cannot encrypt: result code {code}"))
    }

    fn decrypt(&mut self, sender: u64, frame: &[u8]) -> Result<Vec<u8>, i32> {
        self.decryptors
            .get_mut(&sender)
            .expect("a decryptor for the sender")
            .decrypt(frame)
    }

    fn assert_no_failures(&self) {
        assert_eq!(self.session.failures(), Vec::<String>::new());
    }
}

/// Sends frames from a cordial member to a libdave member.
fn cordial_to_libdave(from: &mut CordialMember, from_id: u64, to: &mut LibdaveMember) {
    for frame in opus_frames(FRAME_COUNT) {
        let encrypted = from.encrypt(&frame);
        assert_ne!(encrypted, frame);
        assert_eq!(to.decrypt(from_id, &encrypted), Ok(frame));
    }
}

/// Sends frames from a libdave member to a cordial member.
fn libdave_to_cordial(from: &mut LibdaveMember, to: &mut CordialMember) {
    for frame in opus_frames(FRAME_COUNT) {
        let encrypted = from.encrypt(&frame);
        assert_ne!(encrypted, frame);
        assert_eq!(to.decrypt(from.user_id, &encrypted).unwrap(), frame);
    }
}

/// Sends frames between two cordial members.
fn cordial_to_cordial(from: &mut CordialMember, from_id: u64, to: &mut CordialMember) {
    for frame in opus_frames(FRAME_COUNT) {
        let encrypted = from.encrypt(&frame);
        assert_eq!(to.decrypt(from_id, &encrypted).unwrap(), frame);
    }
}

/// Makes a group: cordial Alice commits the Add of libdave Bob (epoch 1).
fn cordial_commits_for_libdave(gateway: &mut TestGateway) -> (CordialMember, LibdaveMember) {
    let (mut alice, _) = CordialMember::join(gateway, ALICE, &[BOB]);
    let (mut bob, bob_key_package) = LibdaveMember::join(gateway, BOB);

    let (commit, welcome) = alice.proposals(&gateway.propose_adds(&[&bob_key_package]));
    let welcome = welcome.expect("a Welcome for Bob");
    assert_eq!(alice.commit(1, &commit), [ready(1)]);
    let roster = bob
        .session
        .process_welcome(&welcome, &[ALICE, BOB])
        .expect("libdave accepts the Welcome of cordial");
    assert!(roster.contains(&ALICE), "roster {roster:?}");
    gateway.epoch = 1;

    alice.execute(1);
    bob.use_epoch(&[ALICE]);
    (alice, bob)
}

#[test]
#[ignore = "needs libdave: set LIBDAVE_PATH"]
fn both_support_protocol_version_1() {
    assert!(libdave::max_supported_protocol_version() >= 1);
    assert_eq!(DaveSession::new().unwrap().max_protocol_version(), 1);
}

#[test]
#[ignore = "needs libdave: set LIBDAVE_PATH"]
fn libdave_joins_a_group_that_cordial_commits() {
    let mut gateway = TestGateway::new(CHANNEL);
    let (mut alice, mut bob) = cordial_commits_for_libdave(&mut gateway);

    cordial_to_libdave(&mut alice, ALICE, &mut bob);
    libdave_to_cordial(&mut bob, &mut alice);
    bob.assert_no_failures();
}

#[test]
#[ignore = "needs libdave: set LIBDAVE_PATH"]
fn cordial_joins_a_group_that_libdave_commits() {
    let gateway = TestGateway::new(CHANNEL);
    let (mut alice, _) = LibdaveMember::join(&gateway, ALICE);
    let (mut bob, bob_key_package) = CordialMember::join(&gateway, BOB, &[ALICE]);

    let commit_welcome = alice
        .session
        .process_proposals(&gateway.propose_adds(&[&bob_key_package]), &[ALICE, BOB])
        .expect("libdave commits the Add of the cordial key package");
    let (commit, welcome) = split_commit_welcome(&commit_welcome);
    let CommitResult::Applied(roster) = alice.session.process_commit(&commit) else {
        panic!("libdave did not apply its own commit");
    };
    assert!(roster.contains(&BOB), "roster {roster:?}");
    assert_eq!(
        bob.welcome(1, &welcome.expect("a Welcome for Bob")),
        [ready(1)]
    );

    bob.execute(1);
    alice.use_epoch(&[BOB]);
    libdave_to_cordial(&mut alice, &mut bob);
    cordial_to_libdave(&mut bob, BOB, &mut alice);
    alice.assert_no_failures();
}

#[test]
#[ignore = "needs libdave: set LIBDAVE_PATH"]
fn mixed_group_commits_from_both_sides_and_removes_a_member() {
    let mut gateway = TestGateway::new(CHANNEL);
    let (mut alice, _) = CordialMember::join(&gateway, ALICE, &[BOB, CAROL]);
    let (mut carol, carol_key_package) = CordialMember::join(&gateway, CAROL, &[ALICE, BOB]);

    // Epoch 1: cordial Alice adds libdave Bob.
    let (mut bob, bob_key_package) = LibdaveMember::join(&gateway, BOB);
    let (commit, welcome) = alice.proposals(&gateway.propose_adds(&[&bob_key_package]));
    assert_eq!(alice.commit(1, &commit), [ready(1)]);
    bob.session
        .process_welcome(&welcome.unwrap(), &[ALICE, BOB, CAROL])
        .expect("libdave accepts the Welcome of cordial");
    gateway.epoch = 1;
    alice.execute(1);

    // Epoch 2: both members commit the Add of Carol. The gateway selects the libdave commit.
    let proposals = gateway.propose_adds(&[&carol_key_package]);
    let _ = alice.proposals(&proposals);
    let commit_welcome = bob
        .session
        .process_proposals(&proposals, &[ALICE, BOB, CAROL])
        .expect("libdave commits the Add of Carol");
    let (commit, welcome) = split_commit_welcome(&commit_welcome);
    assert_eq!(alice.commit(2, &commit), [ready(2)]);
    let CommitResult::Applied(roster) = bob.session.process_commit(&commit) else {
        panic!("libdave did not apply its own commit");
    };
    assert!(roster.contains(&CAROL), "roster {roster:?}");
    assert_eq!(
        carol.welcome(2, &welcome.expect("a Welcome for Carol")),
        [ready(2)]
    );
    gateway.epoch = 2;
    alice.execute(2);
    carol.execute(2);
    bob.use_epoch(&[ALICE, CAROL]);

    cordial_to_libdave(&mut alice, ALICE, &mut bob);
    cordial_to_libdave(&mut carol, CAROL, &mut bob);
    libdave_to_cordial(&mut bob, &mut alice);
    libdave_to_cordial(&mut bob, &mut carol);
    cordial_to_cordial(&mut alice, ALICE, &mut carol);
    cordial_to_cordial(&mut carol, CAROL, &mut alice);

    // Epoch 3: Carol leaves, and the gateway proposes her removal (leaf 2) to all members. The
    // gateway selects the cordial commit.
    alice.session.client_disconnect(user(CAROL));
    let proposals = gateway.propose_remove(2);
    let (commit, welcome) = alice.proposals(&proposals);
    assert!(welcome.is_none());
    assert!(
        bob.session
            .process_proposals(&proposals, &[ALICE, BOB])
            .is_some()
    );
    // Carol cannot commit her own removal.
    assert!(carol.session.binary_message(27, &proposals).is_err());

    assert_eq!(alice.commit(3, &commit), [ready(3)]);
    let CommitResult::Applied(roster) = bob.session.process_commit(&commit) else {
        panic!("libdave did not apply the cordial commit");
    };
    assert!(roster.contains(&CAROL), "roster {roster:?}");
    assert!(carol.commit(3, &commit).is_empty());
    alice.execute(3);
    bob.use_epoch(&[ALICE]);

    cordial_to_libdave(&mut alice, ALICE, &mut bob);
    libdave_to_cordial(&mut bob, &mut alice);
    let after_removal = alice.encrypt(&opus_frames(1)[0]);
    assert!(carol.decrypt(ALICE, &after_removal).is_err());
    bob.assert_no_failures();
}

#[test]
#[ignore = "needs libdave: set LIBDAVE_PATH"]
fn frames_out_of_order_pass_and_replays_fail() {
    let mut gateway = TestGateway::new(CHANNEL);
    let (mut alice, mut bob) = cordial_commits_for_libdave(&mut gateway);
    let frames = opus_frames(64);

    let from_bob: Vec<Vec<u8>> = frames.iter().map(|frame| bob.encrypt(frame)).collect();
    for (frame, encrypted) in frames.iter().zip(&from_bob).rev() {
        assert_eq!(&alice.decrypt(BOB, encrypted).unwrap(), frame);
    }
    assert!(
        alice.decrypt(BOB, &from_bob[0]).is_err(),
        "cordial refuses a replay"
    );

    let from_alice: Vec<Vec<u8>> = frames.iter().map(|frame| alice.encrypt(frame)).collect();
    for (frame, encrypted) in frames.iter().zip(&from_alice).rev() {
        assert_eq!(&bob.decrypt(ALICE, encrypted).unwrap(), frame);
    }
    assert!(
        bob.decrypt(ALICE, &from_alice[0]).is_err(),
        "libdave refuses a replay"
    );
}

#[test]
#[ignore = "needs libdave: set LIBDAVE_PATH"]
fn silence_frames_pass_both_ways() {
    let mut gateway = TestGateway::new(CHANNEL);
    let (mut alice, mut bob) = cordial_commits_for_libdave(&mut gateway);

    let from_alice = alice.encrypt(&OPUS_SILENCE);
    assert_eq!(from_alice, OPUS_SILENCE);
    assert_eq!(bob.decrypt(ALICE, &from_alice), Ok(OPUS_SILENCE.to_vec()));

    let from_bob = bob.encrypt(&OPUS_SILENCE);
    assert_eq!(alice.decrypt(BOB, &from_bob).unwrap(), OPUS_SILENCE);
}
