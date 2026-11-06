//! Fixture tests for users, emojis, stickers, and guilds.

mod common;

use common::assert_round_trip;
use cordial_model::emoji::Emoji;
use cordial_model::guild::{
    Ban, CurrentUserGuild, Guild, GuildFeature, GuildPreview, GuildWidget, GuildWidgetSettings,
    IncidentsData, Member, Onboarding, Role, UnavailableGuild, WelcomeScreen,
};
use cordial_model::sticker::{Sticker, StickerPack};
use cordial_model::user::User;

#[test]
fn user() {
    let user: User = assert_round_trip("resources/user/example-user");

    assert_eq!(user.username, "Nelly");
}

#[test]
fn current_user_guild() {
    let guild: CurrentUserGuild = assert_round_trip("resources/user/example-partial-guild");

    assert!(guild.owner);
}

#[test]
fn emojis() {
    for path in [
        "resources/emoji/emoji-example",
        "resources/emoji/standard-emoji-example",
        "resources/emoji/custom-emoji-examples",
        "resources/emoji/custom-emoji-examples-2",
    ] {
        assert_round_trip::<Emoji>(path);
    }
}

#[test]
fn stickers() {
    assert_round_trip::<Sticker>("resources/sticker/example-sticker");
    assert_round_trip::<StickerPack>("resources/sticker/example-sticker-pack");
}

#[test]
fn guild() {
    let guild: Guild = assert_round_trip("resources/guild/example-guild");

    assert!(guild.has_feature(&GuildFeature::AnimatedIcon));
}

#[test]
fn guild_parts() {
    assert_round_trip::<Ban>("resources/guild/example-ban");
    // The docs example has `"user": {}` as a placeholder.
    let mut member = common::read_fixture("resources/guild/example-guild-member");
    member.as_object_mut().unwrap().remove("user");
    common::assert_value_round_trip::<Member>("example-guild-member", &member);
    assert_round_trip::<Onboarding>("resources/guild/example-guild-onboarding");
    assert_round_trip::<GuildPreview>("resources/guild/example-guild-preview");
    assert_round_trip::<GuildWidgetSettings>("resources/guild/example-guild-widget-settings");
    assert_round_trip::<GuildWidget>("resources/guild/example-guild-widget");
    assert_round_trip::<IncidentsData>("resources/guild/example-incidents-data");
    assert_round_trip::<UnavailableGuild>("resources/guild/example-unavailable-guild");
    assert_round_trip::<WelcomeScreen>("resources/guild/example-welcome-screen");
    assert_round_trip::<Role>("topics/permissions/example-role");
}
