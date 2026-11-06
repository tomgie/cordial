//! Fixture tests for gateway payloads and events.

mod common;

use common::{assert_round_trip, read_fixture};
use cordial_model::gateway::event::VoiceServerUpdate;
use cordial_model::gateway::presence::{Activity, Status};
use cordial_model::gateway::{
    Hello, OpCode, RequestGuildMembers, RequestSoundboardSounds, SendPayload, UpdatePresence,
};
use cordial_model::id::GuildId;

#[test]
fn activities() {
    assert_round_trip::<Activity>("events/gateway-events/example-activity");
    assert_round_trip::<Activity>("events/gateway-events/example-activity-with-rich-presence");
}

#[test]
fn receive_payloads() {
    assert_round_trip::<VoiceServerUpdate>(
        "events/gateway-events/example-voice-server-update-payload",
    );

    let hello = read_fixture("events/gateway-events/example-hello");
    let data: Hello = serde_json::from_value(hello["d"].clone()).unwrap();
    assert_eq!(data.heartbeat_interval, 45000);
}

/// Encodes a send payload and compares it with the docs example.
fn assert_encodes_like<T: serde::Serialize>(path: &str, payload: &SendPayload<T>) {
    let expected = read_fixture(path);
    let actual = serde_json::to_value(payload).unwrap();

    assert_eq!(actual, expected, "{path}");
}

#[test]
fn send_payloads() {
    let mut presence = UpdatePresence::new(
        Status::Online,
        vec![Activity::playing("Save the Oxford Comma")],
    );
    presence.since = Some(91_879_201);
    assert_encodes_like(
        "events/gateway-events/example-gateway-presence-update",
        &SendPayload::new(OpCode::PresenceUpdate, presence),
    );

    let members = RequestGuildMembers::all(GuildId::new_const(41_771_983_444_115_456));
    assert!(members.is_all_members());
    assert_encodes_like(
        "events/gateway-events/example-request-guild-members",
        &SendPayload::new(OpCode::RequestGuildMembers, members),
    );

    let sounds = RequestSoundboardSounds::new(vec![
        GuildId::new_const(613_425_648_685_547_541),
        GuildId::new_const(81_384_788_765_712_384),
    ]);
    assert_encodes_like(
        "events/gateway-events/example-request-soundboard-sounds",
        &SendPayload::new(OpCode::RequestSoundboardSounds, sounds),
    );
}
