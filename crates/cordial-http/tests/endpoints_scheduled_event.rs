//! Tests of the scheduled event and guild template endpoints.

#[path = "common_guild/mod.rs"]
mod common;

use common::{
    GUILD, assert_reason, member_json, scheduled_event_json, setup_empty, setup_json,
    single_request, user_json,
};
use cordial_http::Error;
use cordial_model::Timestamp;
use cordial_model::id::{ChannelId, GenericId, ScheduledEventId, UserId};
use cordial_model::scheduled_event::{
    ScheduledEventEntityType, ScheduledEventStatus, ScheduledEventUser,
};
use serde_json::json;

const EVENT: ScheduledEventId = ScheduledEventId::new_const(900);

fn time(text: &str) -> Timestamp {
    Timestamp::parse(text).unwrap()
}

#[tokio::test]
async fn list_scheduled_events() {
    let (client, transport) = setup_json(&json!([scheduled_event_json()]));

    let events = client
        .guild_scheduled_events(GUILD)
        .with_user_count(true)
        .await
        .unwrap();

    single_request(
        &transport,
        "GET",
        "/guilds/100/scheduled-events?with_user_count=true",
    );
    assert_eq!(events[0].user_count, Some(12));
}

#[tokio::test]
async fn create_external_scheduled_event() {
    let (client, transport) = setup_json(&scheduled_event_json());

    let event = client
        .create_guild_scheduled_event(
            GUILD,
            "Game night",
            ScheduledEventEntityType::External,
            time("2026-12-01T20:00:00+00:00"),
        )
        .location("Main hall")
        .scheduled_end_time(time("2026-12-01T23:00:00+00:00"))
        .description("Bring snacks")
        .reason("event")
        .await
        .unwrap();

    let request = single_request(&transport, "POST", "/guilds/100/scheduled-events");
    assert_reason(&request, "event");
    let body = request.json();
    assert_eq!(body["name"], json!("Game night"));
    assert_eq!(body["privacy_level"], json!(2));
    assert_eq!(body["entity_type"], json!(3));
    assert_eq!(body["entity_metadata"], json!({"location": "Main hall"}));
    assert_eq!(body["description"], json!("Bring snacks"));
    assert!(
        body["scheduled_start_time"]
            .as_str()
            .unwrap()
            .starts_with("2026-12-01T20:00:00")
    );
    assert!(
        body["scheduled_end_time"]
            .as_str()
            .unwrap()
            .starts_with("2026-12-01T23:00:00")
    );
    assert!(body.get("channel_id").is_none());
    assert_eq!(event.id, EVENT);
}

#[tokio::test]
async fn create_external_scheduled_event_needs_location() {
    let (client, transport) = setup_json(&scheduled_event_json());

    let result = client
        .create_guild_scheduled_event(
            GUILD,
            "Game night",
            ScheduledEventEntityType::External,
            time("2026-12-01T20:00:00+00:00"),
        )
        .scheduled_end_time(time("2026-12-01T23:00:00+00:00"))
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn create_voice_scheduled_event_needs_channel() {
    let (client, transport) = setup_json(&scheduled_event_json());

    let result = client
        .create_guild_scheduled_event(
            GUILD,
            "Voice chat",
            ScheduledEventEntityType::Voice,
            time("2026-12-01T20:00:00+00:00"),
        )
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn get_scheduled_event() {
    let (client, transport) = setup_json(&scheduled_event_json());

    let event = client
        .guild_scheduled_event(GUILD, EVENT)
        .with_user_count(false)
        .await
        .unwrap();

    single_request(
        &transport,
        "GET",
        "/guilds/100/scheduled-events/900?with_user_count=false",
    );
    assert_eq!(event.name, "Game night");
}

#[tokio::test]
async fn edit_scheduled_event() {
    let (client, transport) = setup_json(&scheduled_event_json());

    client
        .edit_guild_scheduled_event(GUILD, EVENT)
        .status(ScheduledEventStatus::Active)
        .channel_id(ChannelId::new_const(5))
        .entity_type(ScheduledEventEntityType::Voice)
        .clear_entity_metadata()
        .clear_description()
        .clear_recurrence_rule()
        .reason("start")
        .await
        .unwrap();

    let request = single_request(&transport, "PATCH", "/guilds/100/scheduled-events/900");
    assert_reason(&request, "start");
    assert_eq!(
        request.json(),
        json!({
            "channel_id": "5", "entity_metadata": null, "description": null,
            "entity_type": 2, "status": 2, "recurrence_rule": null
        })
    );
}

#[tokio::test]
async fn delete_scheduled_event() {
    let (client, transport) = setup_empty();

    client
        .delete_guild_scheduled_event(GUILD, EVENT)
        .await
        .unwrap();

    single_request(&transport, "DELETE", "/guilds/100/scheduled-events/900");
}

fn event_user_json() -> serde_json::Value {
    json!({
        "guild_scheduled_event_id": "900",
        "user": user_json("53908099506183680"),
        "member": member_json()
    })
}

#[tokio::test]
async fn get_scheduled_event_users() {
    let (client, transport) = setup_json(&json!([event_user_json()]));

    let users: Vec<ScheduledEventUser> = client
        .guild_scheduled_event_users(GUILD, EVENT)
        .limit(250)
        .with_member(true)
        .before(UserId::new_const(77))
        .await
        .unwrap();

    single_request(
        &transport,
        "GET",
        "/guilds/100/scheduled-events/900/users?limit=100&with_member=true&before=77",
    );
    assert!(users[0].member.is_some());
}

#[tokio::test]
async fn get_scheduled_event_user_counts() {
    let (client, transport) = setup_json(&json!({
        "guild_scheduled_event_count": 12,
        "guild_scheduled_event_exception_counts": {"31": 4, "32": 0}
    }));

    let counts = client
        .guild_scheduled_event_user_counts(GUILD, EVENT)
        .exception_ids(vec![GenericId::new_const(31), GenericId::new_const(32)])
        .await
        .unwrap();

    single_request(
        &transport,
        "GET",
        "/guilds/100/scheduled-events/900/users/counts?guild_scheduled_event_exception_ids=31&guild_scheduled_event_exception_ids=32",
    );
    assert_eq!(counts.guild_scheduled_event_count, 12);
    assert_eq!(
        counts
            .guild_scheduled_event_exception_counts
            .get(&GenericId::new_const(31)),
        Some(&4)
    );
}

#[tokio::test]
async fn get_scheduled_event_exception_users() {
    let mut user = event_user_json();
    user["guild_scheduled_event_exception_id"] = json!("31");
    user["user_id"] = json!("53908099506183680");
    user["response"] = json!(1);
    let (client, transport) = setup_json(&json!([user]));

    let users = client
        .guild_scheduled_event_exception_users(GUILD, EVENT, GenericId::new_const(31))
        .after(UserId::new_const(5))
        .await
        .unwrap();

    single_request(
        &transport,
        "GET",
        "/guilds/100/scheduled-events/900/31/users?after=5",
    );
    assert_eq!(users.len(), 1);
}

fn exception_json(canceled: bool) -> serde_json::Value {
    json!({
        "event_id": "900", "event_exception_id": "31",
        "scheduled_start_time": null, "scheduled_end_time": null, "is_canceled": canceled
    })
}

#[tokio::test]
async fn create_scheduled_event_exception() {
    let (client, transport) = setup_json(&exception_json(true));

    let exception = client
        .create_guild_scheduled_event_exception(GUILD, EVENT, time("2026-12-08T20:00:00+00:00"))
        .canceled(true)
        .await
        .unwrap();

    let request = single_request(
        &transport,
        "POST",
        "/guilds/100/scheduled-events/900/exceptions",
    );
    let body = request.json();
    assert!(
        body["original_scheduled_start_time"]
            .as_str()
            .unwrap()
            .starts_with("2026-12-08T20:00:00")
    );
    assert_eq!(body["is_canceled"], json!(true));
    assert_eq!(body.as_object().unwrap().len(), 2);
    assert!(exception.is_canceled);
    assert_eq!(exception.event_exception_id, GenericId::new_const(31));
}

#[tokio::test]
async fn edit_scheduled_event_exception() {
    let (client, transport) = setup_json(&exception_json(false));

    client
        .edit_guild_scheduled_event_exception(GUILD, EVENT, GenericId::new_const(31))
        .scheduled_start_time(time("2026-12-09T20:00:00+00:00"))
        .clear_scheduled_end_time()
        .canceled(false)
        .await
        .unwrap();

    let request = single_request(
        &transport,
        "PATCH",
        "/guilds/100/scheduled-events/900/exceptions/31",
    );
    let body = request.json();
    assert!(
        body["scheduled_start_time"]
            .as_str()
            .unwrap()
            .starts_with("2026-12-09")
    );
    assert_eq!(body["scheduled_end_time"], json!(null));
    assert_eq!(body["is_canceled"], json!(false));
}

#[tokio::test]
async fn delete_scheduled_event_exception() {
    let (client, transport) = setup_empty();

    client
        .delete_guild_scheduled_event_exception(GUILD, EVENT, GenericId::new_const(31))
        .await
        .unwrap();

    single_request(
        &transport,
        "DELETE",
        "/guilds/100/scheduled-events/900/exceptions/31",
    );
}

/// The example guild template from the docs.
fn template_json() -> serde_json::Value {
    json!({
        "code": "hgM48av5Q69A",
        "name": "Friends & Family",
        "description": "",
        "usage_count": 49605,
        "creator_id": "132837293881950208",
        "creator": {
            "id": "132837293881950208", "username": "hoges",
            "avatar": "79b0d9f8c340f2d43e1f78b09f175b62", "discriminator": "0001",
            "public_flags": 129
        },
        "created_at": "2020-04-02T21:10:38+00:00",
        "updated_at": "2020-05-01T17:57:38+00:00",
        "source_guild_id": "678070694164299796",
        "serialized_source_guild": {
            "name": "Friends & Family", "description": null, "region": "us-west",
            "verification_level": 0, "default_message_notifications": 0,
            "explicit_content_filter": 0, "preferred_locale": "en-US", "afk_timeout": 300,
            "roles": [{"id": 0, "name": "@everyone", "permissions": 104_324_689, "color": 0,
                       "hoist": false, "mentionable": false}],
            "channels": [{"name": "general", "position": 1, "topic": null, "bitrate": 64000,
                          "user_limit": 0, "nsfw": false, "rate_limit_per_user": 0,
                          "parent_id": 1, "permission_overwrites": [], "id": 2, "type": 0}],
            "afk_channel_id": null, "system_channel_id": 2, "system_channel_flags": 0,
            "icon_hash": null
        },
        "is_dirty": null
    })
}

#[tokio::test]
async fn get_guild_template() {
    let (client, transport) = setup_json(&template_json());

    let template = client.guild_template("hgM48av5Q69A").await.unwrap();

    let request = single_request(&transport, "GET", "/guilds/templates/hgM48av5Q69A");
    assert_eq!(request.header("authorization"), Some("Bot abc"));
    assert_eq!(template.usage_count, 49605);
    assert_eq!(template.creator.unwrap().username, "hoges");
    assert_eq!(template.serialized_source_guild["roles"][0]["id"], json!(0));
}

#[tokio::test]
async fn get_guild_template_encodes_code() {
    let (client, transport) = setup_json(&template_json());

    client.guild_template("a/b").await.unwrap();

    single_request(&transport, "GET", "/guilds/templates/a%2Fb");
}

#[tokio::test]
async fn get_guild_templates() {
    let (client, transport) = setup_json(&json!([template_json()]));

    let templates = client.guild_templates(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/templates");
    assert_eq!(templates[0].code, "hgM48av5Q69A");
}

#[tokio::test]
async fn create_guild_template() {
    let (client, transport) = setup_json(&template_json());

    client
        .create_guild_template(GUILD, "Friends & Family")
        .description("For friends")
        .await
        .unwrap();

    let request = single_request(&transport, "POST", "/guilds/100/templates");
    assert_eq!(
        request.json(),
        json!({"name": "Friends & Family", "description": "For friends"})
    );
}

#[tokio::test]
async fn create_guild_template_refuses_long_description() {
    let (client, transport) = setup_json(&template_json());

    let result = client
        .create_guild_template(GUILD, "Name")
        .description("a".repeat(121))
        .await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn sync_guild_template() {
    let (client, transport) = setup_json(&template_json());

    client
        .sync_guild_template(GUILD, "hgM48av5Q69A")
        .await
        .unwrap();

    let request = single_request(&transport, "PUT", "/guilds/100/templates/hgM48av5Q69A");
    assert!(request.body.is_empty());
}

#[tokio::test]
async fn edit_guild_template() {
    let (client, transport) = setup_json(&template_json());

    client
        .edit_guild_template(GUILD, "hgM48av5Q69A")
        .name("Family")
        .clear_description()
        .await
        .unwrap();

    let request = single_request(&transport, "PATCH", "/guilds/100/templates/hgM48av5Q69A");
    assert_eq!(
        request.json(),
        json!({"name": "Family", "description": null})
    );
}

#[tokio::test]
async fn delete_guild_template() {
    let (client, transport) = setup_json(&template_json());

    let template = client
        .delete_guild_template(GUILD, "hgM48av5Q69A")
        .await
        .unwrap();

    single_request(&transport, "DELETE", "/guilds/100/templates/hgM48av5Q69A");
    assert_eq!(template.name, "Friends & Family");
}
