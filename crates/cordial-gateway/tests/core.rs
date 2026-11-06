//! Tests of the sans-IO shard core with scripted frames and synthetic time.

use std::time::{Duration, Instant};

use cordial_gateway::core::{
    Action, Command, CommandError, CoreConfig, FatalError, Lifecycle, ReconnectKind, Session,
    ShardCore,
};
use cordial_model::gateway::{Intents, RequestGuildMembers};
use cordial_model::id::GuildId;
use serde_json::Value;

const HELLO: &str = r#"{"op":10,"d":{"heartbeat_interval":45000},"s":null,"t":null}"#;
const READY: &str = r#"{"op":0,"s":1,"t":"READY","d":{"v":10,"user":{"id":"1","username":"bot","discriminator":"0","avatar":null},"guilds":[],"session_id":"abc","resume_gateway_url":"wss://resume.example","application":{"id":"1","flags":0}}}"#;

fn config() -> CoreConfig {
    CoreConfig::new("TOKEN", Intents::GUILDS | Intents::GUILD_MESSAGES)
}

fn drain(core: &mut ShardCore) -> Vec<Action> {
    std::iter::from_fn(|| core.poll_action()).collect()
}

fn sent(actions: &[Action]) -> Vec<Value> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::Send(text) => Some(serde_json::from_str(text).unwrap()),
            _ => None,
        })
        .collect()
}

/// Connects, receives Hello, identifies, and receives READY.
fn ready_core(now: Instant) -> ShardCore {
    let mut core = ShardCore::new(config(), now);
    core.connected(now);
    core.received(HELLO, now).unwrap();
    core.identify(now);
    core.received(READY, now).unwrap();
    drain(&mut core);
    core
}

#[test]
fn identifies_after_hello_and_permit() {
    let now = Instant::now();
    let mut core = ShardCore::new(config(), now);
    core.connected(now);

    core.received(HELLO, now).unwrap();
    let actions = drain(&mut core);
    assert!(actions.contains(&Action::RequestIdentify));
    assert!(sent(&actions).is_empty());

    core.identify(now);
    let payloads = sent(&drain(&mut core));
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0]["op"], 2);
    assert_eq!(payloads[0]["d"]["token"], "TOKEN");
    assert_eq!(payloads[0]["d"]["intents"], 513);
    assert_eq!(payloads[0]["d"]["shard"], serde_json::json!([0, 1]));
}

#[test]
fn ready_stores_session_and_emits_dispatch() {
    let now = Instant::now();
    let mut core = ShardCore::new(config(), now);
    core.connected(now);
    core.received(HELLO, now).unwrap();
    core.identify(now);
    drain(&mut core);

    core.received(READY, now).unwrap();
    let actions = drain(&mut core);

    assert!(actions.iter().any(|action| matches!(action, Action::Lifecycle(Lifecycle::Ready { session_id }) if session_id == "abc")));
    assert!(
        actions
            .iter()
            .any(|action| matches!(action, Action::Dispatch { name, .. } if name == "READY"))
    );
    assert!(core.is_active());
    let session = core.session().unwrap();
    assert_eq!(session.resume_url, "wss://resume.example");
    assert_eq!(session.sequence, 1);
}

#[test]
fn heartbeats_with_jitter_then_interval() {
    let now = Instant::now();
    let mut core = ready_core(now);

    let first_due = core.next_deadline().unwrap();
    assert!(first_due <= now + Duration::from_secs(45));

    core.tick(first_due);
    let payloads = sent(&drain(&mut core));
    assert_eq!(payloads, vec![serde_json::json!({"op": 1, "d": 1})]);
    assert_eq!(
        core.next_deadline(),
        Some(first_due + Duration::from_secs(45))
    );
}

#[test]
fn missing_ack_is_a_zombie() {
    let now = Instant::now();
    let mut core = ready_core(now);
    let first_due = core.next_deadline().unwrap();
    core.tick(first_due);
    drain(&mut core);

    core.tick(first_due + Duration::from_secs(45));
    let actions = drain(&mut core);

    assert!(actions.contains(&Action::Lifecycle(Lifecycle::Zombie)));
    assert!(actions.iter().any(|action| matches!(action, Action::Reconnect { kind: ReconnectKind::Resume, close_code, .. } if *close_code != 1000 && *close_code != 1001)));
}

#[test]
fn ack_measures_latency() {
    let now = Instant::now();
    let mut core = ready_core(now);
    let due = core.next_deadline().unwrap();
    core.tick(due);
    drain(&mut core);

    core.received(r#"{"op":11}"#, due + Duration::from_millis(80))
        .unwrap();

    assert_eq!(core.latency(), Some(Duration::from_millis(80)));
    core.tick(due + Duration::from_secs(45));
    assert_eq!(
        sent(&drain(&mut core)).len(),
        1,
        "a new heartbeat, not a zombie"
    );
}

#[test]
fn server_heartbeat_request_gets_immediate_heartbeat() {
    let now = Instant::now();
    let mut core = ready_core(now);

    core.received(r#"{"op":1,"d":null}"#, now).unwrap();

    assert_eq!(sent(&drain(&mut core))[0]["op"], 1);
}

#[test]
fn resumes_with_session() {
    let now = Instant::now();
    let session = Session::new("abc".to_owned(), 42, "wss://resume.example".to_owned());
    let mut core = ShardCore::with_session(config(), session, now);
    core.connected(now);

    core.received(HELLO, now).unwrap();
    let payloads = sent(&drain(&mut core));

    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0]["op"], 6);
    assert_eq!(payloads[0]["d"]["session_id"], "abc");
    assert_eq!(payloads[0]["d"]["seq"], 42);
}

#[test]
fn reconnect_opcode_resumes() {
    let now = Instant::now();
    let mut core = ready_core(now);

    core.received(r#"{"op":7,"d":null}"#, now).unwrap();
    let actions = drain(&mut core);

    assert!(actions.iter().any(|action| matches!(
        action,
        Action::Reconnect {
            kind: ReconnectKind::Resume,
            ..
        }
    )));
}

#[test]
fn invalid_session_false_identifies_after_delay() {
    let now = Instant::now();
    let mut core = ready_core(now);

    core.received(r#"{"op":9,"d":false}"#, now).unwrap();
    let actions = drain(&mut core);

    let reconnect = actions.iter().find_map(|action| match action {
        Action::Reconnect { kind, delay, .. } => Some((*kind, *delay)),
        _ => None,
    });
    let (kind, delay) = reconnect.unwrap();
    assert_eq!(kind, ReconnectKind::Identify);
    assert!(delay >= Duration::from_secs(1) && delay <= Duration::from_secs(5));
    assert!(core.session().is_none());
}

#[test]
fn invalid_session_true_resumes() {
    let now = Instant::now();
    let mut core = ready_core(now);

    core.received(r#"{"op":9,"d":true}"#, now).unwrap();

    assert!(drain(&mut core).iter().any(|action| matches!(
        action,
        Action::Reconnect {
            kind: ReconnectKind::Resume,
            ..
        }
    )));
}

#[test]
fn close_codes() {
    let now = Instant::now();

    let mut fatal = ready_core(now);
    fatal.closed(Some(4014), now);
    let actions = drain(&mut fatal);
    assert!(actions.contains(&Action::Fatal(FatalError::Closed { code: 4014 })));
    assert!(fatal.is_stopped());
    let message = FatalError::Closed { code: 4014 }.to_string();
    assert!(message.contains("privileged intent"));

    let mut new_session = ready_core(now);
    new_session.closed(Some(4009), now);
    assert!(drain(&mut new_session).iter().any(|action| matches!(
        action,
        Action::Reconnect {
            kind: ReconnectKind::Identify,
            ..
        }
    )));

    let mut resume = ready_core(now);
    resume.closed(None, now);
    assert!(drain(&mut resume).iter().any(|action| matches!(
        action,
        Action::Reconnect {
            kind: ReconnectKind::Resume,
            ..
        }
    )));
}

#[test]
fn commands_respect_send_limit() {
    let now = Instant::now();
    let mut core = ready_core(now);
    // READY used 1 send (Identify). User commands can use 117 in the window.
    for index in 0..200_u64 {
        let request = RequestGuildMembers::users(GuildId::new_const(index + 1), vec![]);
        core.send_command(&Command::RequestGuildMembers(request), now)
            .unwrap();
    }

    let member_requests = |actions: &[Action]| {
        sent(actions)
            .iter()
            .filter(|payload| payload["op"] == 8)
            .count()
    };

    let first_window = member_requests(&drain(&mut core));
    assert_eq!(first_window, 116);

    core.tick(now + Duration::from_secs(60));
    let second_window = member_requests(&drain(&mut core));
    assert_eq!(second_window, 84);
}

#[test]
fn all_member_requests_wait_30_seconds_per_guild() {
    let now = Instant::now();
    let mut core = ready_core(now);
    let guild = GuildId::new_const(5);

    core.send_command(
        &Command::RequestGuildMembers(RequestGuildMembers::all(guild)),
        now,
    )
    .unwrap();
    let second = core.send_command(
        &Command::RequestGuildMembers(RequestGuildMembers::all(guild)),
        now + Duration::from_secs(10),
    );
    let other_guild = core.send_command(
        &Command::RequestGuildMembers(RequestGuildMembers::all(GuildId::new_const(6))),
        now + Duration::from_secs(10),
    );
    let later = core.send_command(
        &Command::RequestGuildMembers(RequestGuildMembers::all(guild)),
        now + Duration::from_secs(31),
    );

    assert!(
        matches!(second, Err(CommandError::RateLimited { retry_after }) if retry_after == Duration::from_secs(20))
    );
    assert!(other_guild.is_ok());
    assert!(later.is_ok());
}

#[test]
fn refuses_payloads_over_4096_bytes() {
    let now = Instant::now();
    let mut core = ready_core(now);
    let ids = (1..=400)
        .map(|index| cordial_model::id::UserId::new_const(1_000_000_000_000_000_000 + index))
        .collect();

    let result = core.send_command(
        &Command::RequestGuildMembers(RequestGuildMembers::users(GuildId::new_const(1), ids)),
        now,
    );

    assert!(matches!(result, Err(CommandError::TooLarge { .. })));
}

#[test]
fn sequence_never_goes_back() {
    let now = Instant::now();
    let mut core = ready_core(now);

    core.received(r#"{"op":0,"s":10,"t":"TYPING_START","d":{}}"#, now)
        .unwrap();
    core.received(r#"{"op":0,"s":9,"t":"TYPING_START","d":{}}"#, now)
        .unwrap();

    assert_eq!(core.session().unwrap().sequence, 10);
}

fn gaps(actions: &[Action]) -> Vec<(u64, u64)> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::Lifecycle(Lifecycle::SequenceGap { expected, received }) => {
                Some((*expected, *received))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn skipped_sequence_is_a_gap() {
    let now = Instant::now();
    let mut core = ready_core(now);

    core.received(r#"{"op":0,"s":2,"t":"TYPING_START","d":{}}"#, now)
        .unwrap();
    core.received(r#"{"op":0,"s":5,"t":"TYPING_START","d":{}}"#, now)
        .unwrap();

    assert_eq!(gaps(&drain(&mut core)), [(3, 5)]);
}

#[test]
fn new_session_and_old_sequence_are_not_gaps() {
    let now = Instant::now();
    let mut core = ShardCore::new(config(), now);
    core.connected(now);
    core.received(HELLO, now).unwrap();
    core.identify(now);
    let late_ready = READY.replace(r#""s":1"#, r#""s":7"#);
    core.received(&late_ready, now).unwrap();
    core.received(r#"{"op":0,"s":8,"t":"TYPING_START","d":{}}"#, now)
        .unwrap();
    core.received(r#"{"op":0,"s":6,"t":"TYPING_START","d":{}}"#, now)
        .unwrap();

    assert!(gaps(&drain(&mut core)).is_empty());
}

fn member_request(guild: u64) -> Command {
    Command::RequestGuildMembers(RequestGuildMembers::users(
        GuildId::new_const(guild),
        vec![],
    ))
}

fn reconnects(actions: &[Action]) -> usize {
    actions
        .iter()
        .filter(|action| matches!(action, Action::Reconnect { .. }))
        .count()
}

/// A queued command before READY made the deadline the old end of the send window. The
/// driver then woke up again and again without a pause.
#[test]
fn deadline_moves_forward_while_not_active() {
    let now = Instant::now();
    let mut core = ShardCore::new(config(), now);
    core.connected(now);
    core.received(
        r#"{"op":10,"d":{"heartbeat_interval":1000000},"s":null,"t":null}"#,
        now,
    )
    .unwrap();
    core.send_command(&member_request(1), now).unwrap();
    drain(&mut core);

    // Do what the driver loop does: tick at each deadline.
    for _ in 0..3 {
        let Some(deadline) = core.next_deadline() else {
            break;
        };
        core.tick(deadline);
        drain(&mut core);

        assert!(core.next_deadline().is_none_or(|next| next > deadline));
    }
}

/// Without Hello, the shard waited for ever.
#[test]
fn reconnects_when_hello_does_not_arrive() {
    let now = Instant::now();
    let mut core = ShardCore::new(config(), now);
    core.connected(now);

    assert_eq!(core.next_deadline(), Some(now + Duration::from_secs(20)));
    core.tick(now + Duration::from_secs(20));

    assert_eq!(reconnects(&drain(&mut core)), 1);
}

#[test]
fn hello_ends_the_hello_timeout() {
    let now = Instant::now();
    let mut core = ShardCore::new(config(), now);
    core.connected(now);
    core.received(HELLO, now).unwrap();
    drain(&mut core);

    core.tick(now + Duration::from_secs(20));

    assert_eq!(reconnects(&drain(&mut core)), 0);
}

/// Each failed send added a reconnect. The extra ones closed the next connection, and the
/// queued commands were lost.
#[test]
fn failed_send_reconnects_once_and_keeps_commands() {
    let now = Instant::now();
    let mut core = ready_core(now);
    core.send_command(&member_request(1), now).unwrap();
    core.send_command(&member_request(2), now).unwrap();

    // The driver takes the first send, the send fails, and the driver reports it twice.
    assert!(matches!(core.poll_action(), Some(Action::Send(_))));
    core.closed(None, now);
    core.closed(None, now);
    let actions = drain(&mut core);

    assert_eq!(reconnects(&actions), 1);
    assert!(sent(&actions).is_empty());

    core.connected(now);
    core.received(HELLO, now).unwrap();
    core.received(r#"{"op":0,"s":2,"t":"RESUMED","d":null}"#, now)
        .unwrap();
    let payloads = sent(&drain(&mut core));
    let guilds: Vec<&Value> = payloads
        .iter()
        .filter(|payload| payload["op"] == 8)
        .map(|payload| &payload["d"]["guild_id"])
        .collect();
    assert_eq!(guilds, vec!["2"]);
}
