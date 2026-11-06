//! Fixture tests for channels, voice, webhooks, invites, applications, and monetization.

mod common;

use common::{assert_round_trip, assert_round_trip_ignoring};
use cordial_model::application::Application;
use cordial_model::channel::{Channel, ChannelType};
use cordial_model::invite::Invite;
use cordial_model::monetization::{Entitlement, Sku, Subscription};
use cordial_model::voice::{StageInstance, VoiceState};
use cordial_model::webhook::Webhook;

#[test]
fn channels() {
    for path in [
        "resources/channel/example-channel-category",
        "resources/channel/example-dm-channel",
        "resources/channel/example-group-dm-channel",
        "resources/channel/example-guild-announcement-channel",
        "resources/channel/example-guild-text-channel",
        "resources/channel/example-thread-channel",
    ] {
        assert_round_trip::<Channel>(path);
    }

    let thread: Channel = assert_round_trip("resources/channel/example-thread-channel");
    assert_eq!(thread.kind, ChannelType::PublicThread);
    assert!(thread.is_thread());
}

#[test]
fn voice() {
    assert_round_trip::<VoiceState>("resources/voice/example-voice-state");
    assert_round_trip::<StageInstance>("resources/stage-instance/example-stage-instance");
}

#[test]
fn webhooks() {
    for path in [
        "resources/webhook/example-application-webhook",
        "resources/webhook/example-channel-follower-webhook",
        "resources/webhook/example-incoming-webhook",
    ] {
        assert_round_trip::<Webhook>(path);
    }
}

#[test]
fn invites() {
    assert_round_trip::<Invite>("resources/invite/example-invite-object");
}

#[test]
fn application() {
    // `permissions` of team members is an old undocumented field.
    assert_round_trip_ignoring::<Application>(
        "resources/application/example-application-object",
        &["permissions"],
    );
}

#[test]
fn monetization() {
    // The SKU examples have fields that the docs do not define.
    let undocumented = [
        "access_type",
        "dependent_sku_id",
        "features",
        "manifest_labels",
        "premium",
        "release_date",
        "show_age_gate",
    ];
    assert_round_trip_ignoring::<Sku>("resources/sku/sku-example", &undocumented);
    assert_round_trip_ignoring::<Vec<Sku>>("resources/sku/list-skus", &undocumented);
    assert_round_trip::<Entitlement>("resources/entitlement/entitlement-example");
    assert_round_trip::<Entitlement>("resources/entitlement/get-entitlement");
    assert_round_trip::<Subscription>("resources/subscription/subscription-example");
}

#[test]
fn moderation_and_social() {
    use cordial_model::auto_moderation::AutoModerationRule;
    use cordial_model::guild::Integration;
    use cordial_model::lobby::{ApplicationIdentityProfile, Lobby};
    use cordial_model::soundboard::SoundboardSound;

    assert_round_trip::<AutoModerationRule>(
        "resources/auto-moderation/example-auto-moderation-rule",
    );
    assert_round_trip::<Integration>("resources/audit-log/example-partial-integration-object");
    assert_round_trip::<SoundboardSound>("resources/soundboard/example-default-soundboard-sound");
    assert_round_trip::<SoundboardSound>("resources/soundboard/example-guild-soundboard-sound");
    assert_round_trip::<Lobby>("resources/lobby/example-lobby-object");
    assert_round_trip::<ApplicationIdentityProfile>(
        "resources/application-identity-profile/example-application-identity-profile-object",
    );
}
