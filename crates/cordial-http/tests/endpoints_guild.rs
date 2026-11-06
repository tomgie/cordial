//! Tests of the guild endpoints: the guild, channels, prune, settings, and join requests.

#[path = "common_guild/mod.rs"]
mod common;

use bytes::Bytes;
use common::{
    GUILD, assert_reason, channel_json, guild_json, setup, setup_empty, setup_json, single_request,
    user_json,
};
use cordial_http::endpoints::guild::ChannelPosition;
use cordial_http::endpoints::guild_settings::WidgetStyle;
use cordial_http::endpoints::join_request::JoinRequestStatus;
use cordial_http::testing::FakeResponse;
use cordial_http::{Error, ImageData};
use cordial_model::Timestamp;
use cordial_model::channel::ChannelType;
use cordial_model::guild::{GuildFeature, OnboardingMode, VerificationLevel};
use cordial_model::id::{ChannelId, GenericId, IntegrationId, RoleId};
use serde_json::json;

#[tokio::test]
async fn get_guild_with_counts() {
    let (client, transport) = setup_json(&guild_json());

    let guild = client.guild(GUILD).with_counts(true).await.unwrap();

    single_request(&transport, "GET", "/guilds/100?with_counts=true");
    assert_eq!(guild.name, "Mason's Test Server");
    assert_eq!(guild.approximate_member_count, Some(2));
    assert_eq!(guild.roles.len(), 1);
}

#[tokio::test]
async fn get_guild_preview() {
    let (client, transport) = setup_json(&json!({
        "id": "197038439483310086", "name": "Discord Testers",
        "icon": "f64c482b807da4f539cff778d174971c", "splash": null, "discovery_splash": null,
        "emojis": [], "features": ["DISCOVERABLE", "COMMUNITY"],
        "approximate_member_count": 60814, "approximate_presence_count": 20034,
        "description": "The official place to report Discord Bugs!", "stickers": []
    }));

    let preview = client.guild_preview(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/preview");
    assert_eq!(preview.approximate_member_count, 60814);
}

#[tokio::test]
async fn edit_guild() {
    let (client, transport) = setup_json(&guild_json());

    let guild = client
        .edit_guild(GUILD)
        .name("New name")
        .verification_level(VerificationLevel::High)
        .afk_timeout(900)
        .clear_afk_channel_id()
        .icon(ImageData::from_data_uri("data:image/png;base64,AAAA"))
        .clear_banner()
        .features(vec![GuildFeature::Community, GuildFeature::InvitesDisabled])
        .safety_alerts_channel_id(ChannelId::new_const(7))
        .clear_description()
        .premium_progress_bar_enabled(true)
        .reason("cleanup")
        .await
        .unwrap();

    let request = single_request(&transport, "PATCH", "/guilds/100");
    assert_reason(&request, "cleanup");
    assert_eq!(
        request.json(),
        json!({
            "name": "New name", "verification_level": 3, "afk_timeout": 900,
            "afk_channel_id": null, "icon": "data:image/png;base64,AAAA", "banner": null,
            "features": ["COMMUNITY", "INVITES_DISABLED"], "safety_alerts_channel_id": "7",
            "description": null, "premium_progress_bar_enabled": true
        })
    );
    assert_eq!(guild.owner_id.get(), 53_908_232_506_183_680);
}

#[tokio::test]
async fn edit_guild_refuses_bad_afk_timeout() {
    let (client, transport) = setup_json(&guild_json());

    let result = client.edit_guild(GUILD).afk_timeout(42).await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn get_guild_channels() {
    let (client, transport) = setup_json(&json!([channel_json("41771983423143937")]));

    let channels = client.guild_channels(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/channels");
    assert_eq!(channels[0].name.as_deref(), Some("general"));
}

#[tokio::test]
async fn create_guild_channel() {
    let (client, transport) = setup(FakeResponse::json(201, &channel_json("5")));

    let channel = client
        .create_guild_channel(GUILD, "general")
        .kind(ChannelType::GuildText)
        .topic("Talk here")
        .rate_limit_per_user(10)
        .parent_id(ChannelId::new_const(9))
        .nsfw(false)
        .reason("new channel")
        .await
        .unwrap();

    let request = single_request(&transport, "POST", "/guilds/100/channels");
    assert_reason(&request, "new channel");
    assert_eq!(
        request.json(),
        json!({
            "name": "general", "type": 0, "topic": "Talk here", "rate_limit_per_user": 10,
            "parent_id": "9", "nsfw": false
        })
    );
    assert_eq!(channel.id, ChannelId::new_const(5));
}

#[tokio::test]
async fn create_guild_channel_refuses_long_name() {
    let (client, transport) = setup_json(&channel_json("5"));

    let result = client.create_guild_channel(GUILD, "a".repeat(101)).await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn edit_guild_channel_positions() {
    let (client, transport) = setup_empty();
    let positions = vec![
        ChannelPosition::new(ChannelId::new_const(1)).position(0),
        ChannelPosition::new(ChannelId::new_const(2))
            .position(1)
            .parent_id(ChannelId::new_const(3))
            .lock_permissions(true),
        ChannelPosition::new(ChannelId::new_const(4)),
    ];

    client
        .edit_guild_channel_positions(GUILD, positions)
        .reason("reorder")
        .await
        .unwrap();

    let request = single_request(&transport, "PATCH", "/guilds/100/channels");
    assert_reason(&request, "reorder");
    assert_eq!(
        request.json(),
        json!([
            {"id": "1", "position": 0},
            {"id": "2", "position": 1, "lock_permissions": true, "parent_id": "3"},
            {"id": "4"}
        ])
    );
}

#[tokio::test]
async fn edit_guild_channel_positions_refuses_two_parent_changes() {
    let (client, transport) = setup_empty();
    let positions = vec![
        ChannelPosition::new(ChannelId::new_const(1)).parent_id(ChannelId::new_const(3)),
        ChannelPosition::new(ChannelId::new_const(2)).clear_parent_id(),
    ];

    let result = client.edit_guild_channel_positions(GUILD, positions).await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn get_active_guild_threads() {
    let mut thread = channel_json("77");
    thread["type"] = json!(11);
    let (client, transport) = setup_json(&json!({
        "threads": [thread],
        "members": [{"id": "77", "user_id": "5", "join_timestamp": "2026-01-01T00:00:00+00:00", "flags": 0}]
    }));

    let list = client.active_guild_threads(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/threads/active");
    assert_eq!(list.threads.len(), 1);
    assert_eq!(list.members.len(), 1);
}

#[tokio::test]
async fn get_guild_prune_count() {
    let (client, transport) = setup_json(&json!({"pruned": 12}));

    let count = client
        .guild_prune_count(GUILD)
        .days(14)
        .include_roles(vec![RoleId::new_const(1), RoleId::new_const(2)])
        .await
        .unwrap();

    single_request(
        &transport,
        "GET",
        "/guilds/100/prune?days=14&include_roles=1%2C2",
    );
    assert_eq!(count.pruned, Some(12));
}

#[tokio::test]
async fn begin_guild_prune() {
    let (client, transport) = setup_json(&json!({"pruned": null}));

    let count = client
        .begin_guild_prune(GUILD)
        .days(30)
        .compute_prune_count(false)
        .include_roles(vec![RoleId::new_const(3)])
        .reason("inactive")
        .await
        .unwrap();

    let request = single_request(&transport, "POST", "/guilds/100/prune");
    assert_reason(&request, "inactive");
    assert_eq!(
        request.json(),
        json!({"days": 30, "compute_prune_count": false, "include_roles": ["3"]})
    );
    assert_eq!(count.pruned, None);
}

#[tokio::test]
async fn begin_guild_prune_refuses_too_many_days() {
    let (client, transport) = setup_json(&json!({"pruned": null}));

    let result = client.begin_guild_prune(GUILD).days(31).await;

    assert!(matches!(result, Err(Error::Validation(_))));
    assert!(transport.requests().is_empty());
}

#[tokio::test]
async fn get_guild_voice_regions() {
    let (client, transport) = setup_json(&json!([
        {"id": "us-west", "name": "US West", "optimal": true, "deprecated": false, "custom": false}
    ]));

    let regions = client.guild_voice_regions(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/regions");
    assert_eq!(regions[0].id, "us-west");
    assert!(regions[0].optimal);
}

#[tokio::test]
async fn get_guild_invites() {
    let (client, transport) = setup_json(&json!([{
        "type": 0, "code": "0vCdhLbwjZZTWZLD",
        "guild": {"id": "165176875973476352", "name": "CS:GO Fraggers Only", "splash": null,
                  "banner": null, "description": "Some description", "icon": null,
                  "features": [], "verification_level": 2, "vanity_url_code": null,
                  "nsfw_level": 0, "premium_subscription_count": 5},
        "channel": {"id": "165176875973476352", "name": "illuminati", "type": 0},
        "inviter": user_json("115590097100865541"),
        "uses": 3, "max_uses": 0, "max_age": 0, "temporary": false,
        "created_at": "2016-03-31T19:15:39.954000+00:00", "expires_at": null
    }]));

    let invites = client.guild_invites(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/invites");
    assert_eq!(invites[0].code, "0vCdhLbwjZZTWZLD");
    assert_eq!(invites[0].uses, Some(3));
}

#[tokio::test]
async fn get_guild_integrations() {
    let (client, transport) = setup_json(&json!([{
        "id": "33590653072239123", "name": "A Name", "type": "discord", "enabled": true,
        "account": {"id": "33590653072239123", "name": "A Name"}
    }]));

    let integrations = client.guild_integrations(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/integrations");
    assert_eq!(integrations[0].name, "A Name");
}

#[tokio::test]
async fn delete_guild_integration() {
    let (client, transport) = setup_empty();

    client
        .delete_guild_integration(GUILD, IntegrationId::new_const(55))
        .reason("unused")
        .await
        .unwrap();

    let request = single_request(&transport, "DELETE", "/guilds/100/integrations/55");
    assert_reason(&request, "unused");
}

#[tokio::test]
async fn get_guild_vanity_url() {
    let (client, transport) = setup_json(&json!({"code": "abc", "uses": 12}));

    let vanity = client.guild_vanity_url(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/vanity-url");
    assert_eq!(vanity.code.as_deref(), Some("abc"));
    assert_eq!(vanity.uses, 12);
}

#[tokio::test]
async fn edit_guild_incident_actions() {
    let (client, transport) = setup_json(&json!({
        "invites_disabled_until": "2023-09-01T14:48:02.222000+00:00",
        "dms_disabled_until": null
    }));
    let until = Timestamp::parse("2023-09-01T14:48:02.222000+00:00").unwrap();

    let data = client
        .edit_guild_incident_actions(GUILD)
        .invites_disabled_until(until)
        .enable_dms()
        .await
        .unwrap();

    let request = single_request(&transport, "PUT", "/guilds/100/incident-actions");
    let body = request.json();
    assert!(
        body["invites_disabled_until"]
            .as_str()
            .unwrap()
            .starts_with("2023-09-01T14:48:02")
    );
    assert_eq!(body["dms_disabled_until"], json!(null));
    assert_eq!(data.invites_disabled_until, Some(until));
    assert_eq!(data.dms_disabled_until, None);
}

#[tokio::test]
async fn get_guild_widget_settings() {
    let (client, transport) =
        setup_json(&json!({"enabled": true, "channel_id": "41771983444115456"}));

    let settings = client.guild_widget_settings(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/widget");
    assert!(settings.enabled);
    assert_eq!(
        settings.channel_id,
        Some(ChannelId::new_const(41_771_983_444_115_456))
    );
}

#[tokio::test]
async fn edit_guild_widget() {
    let (client, transport) = setup_json(&json!({"enabled": false, "channel_id": null}));

    let settings = client
        .edit_guild_widget(GUILD)
        .enabled(false)
        .clear_channel_id()
        .reason("hide")
        .await
        .unwrap();

    let request = single_request(&transport, "PATCH", "/guilds/100/widget");
    assert_reason(&request, "hide");
    assert_eq!(
        request.json(),
        json!({"enabled": false, "channel_id": null})
    );
    assert!(!settings.enabled);
}

#[tokio::test]
async fn get_guild_widget() {
    let (client, transport) = setup_json(&json!({
        "id": "290926798626999250", "name": "Test Server",
        "instant_invite": "https://discord.com/invite/abcdefg",
        "channels": [{"id": "705216630279993882", "name": "elephant", "position": 2}],
        "members": [{"id": "0", "username": "1234", "discriminator": "0000", "avatar": null,
                     "status": "online", "avatar_url": "https://cdn.discordapp.com/widget-avatars/x"}],
        "presence_count": 1
    }));

    let widget = client.guild_widget(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/widget.json");
    assert_eq!(widget.presence_count, 1);
    assert_eq!(widget.members[0].status, "online");
}

#[tokio::test]
async fn get_guild_widget_image_gives_bytes() {
    let png = Bytes::from_static(b"\x89PNG\r\n\x1a\nrest");
    let mut response = FakeResponse::empty(200).header("content-type", "image/png");
    response.body = png.clone();
    let (client, transport) = setup(response);

    let bytes = client
        .guild_widget_image(GUILD)
        .style(WidgetStyle::Banner2)
        .await
        .unwrap();

    single_request(&transport, "GET", "/guilds/100/widget.png?style=banner2");
    assert_eq!(bytes, png);
}

#[tokio::test]
async fn get_guild_welcome_screen() {
    let (client, transport) = setup_json(&json!({
        "description": "Discord Developers is a place to learn about Discord's API.",
        "welcome_channels": [
            {"channel_id": "697138785317814292", "description": "Follow for official Discord API updates",
             "emoji_id": null, "emoji_name": "📡"}
        ]
    }));

    let screen = client.guild_welcome_screen(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/welcome-screen");
    assert_eq!(screen.welcome_channels[0].emoji_name.as_deref(), Some("📡"));
}

#[tokio::test]
async fn edit_guild_welcome_screen() {
    let (client, transport) = setup_json(&json!({"description": null, "welcome_channels": []}));

    client
        .edit_guild_welcome_screen(GUILD)
        .enabled(true)
        .clear_welcome_channels()
        .description("Hello")
        .reason("welcome")
        .await
        .unwrap();

    let request = single_request(&transport, "PATCH", "/guilds/100/welcome-screen");
    assert_reason(&request, "welcome");
    assert_eq!(
        request.json(),
        json!({"enabled": true, "welcome_channels": null, "description": "Hello"})
    );
}

fn onboarding_json() -> serde_json::Value {
    json!({
        "guild_id": "960007075288915998",
        "prompts": [{
            "id": "1067461047608422473",
            "title": "What do you want to do in this community?",
            "options": [{
                "id": "1067461047608422476", "title": "Chat with Friends", "description": "",
                "emoji": {"id": "1070002302032826408", "name": "chat", "animated": false},
                "role_ids": [], "channel_ids": ["962007075288916001"]
            }],
            "single_select": false, "required": false, "in_onboarding": true, "type": 0
        }],
        "default_channel_ids": ["998678771706110023", "998678693058719784"],
        "enabled": true
    })
}

#[tokio::test]
async fn get_guild_onboarding() {
    let (client, transport) = setup_json(&onboarding_json());

    let onboarding = client.guild_onboarding(GUILD).await.unwrap();

    single_request(&transport, "GET", "/guilds/100/onboarding");
    assert!(onboarding.enabled);
    assert_eq!(onboarding.prompts[0].options[0].title, "Chat with Friends");
}

#[tokio::test]
async fn edit_guild_onboarding() {
    let (client, transport) = setup_json(&onboarding_json());
    let current: cordial_model::guild::Onboarding =
        serde_json::from_value(onboarding_json()).unwrap();

    client
        .edit_guild_onboarding(GUILD)
        .prompts(current.prompts)
        .default_channel_ids(vec![ChannelId::new_const(1), ChannelId::new_const(2)])
        .enabled(true)
        .mode(OnboardingMode::Advanced)
        .reason("onboarding")
        .await
        .unwrap();

    let request = single_request(&transport, "PUT", "/guilds/100/onboarding");
    assert_reason(&request, "onboarding");
    let body = request.json();
    assert_eq!(body["default_channel_ids"], json!(["1", "2"]));
    assert_eq!(body["enabled"], json!(true));
    assert_eq!(body["mode"], json!(1));
    assert_eq!(body["prompts"][0]["id"], json!("1067461047608422473"));
}

#[tokio::test]
async fn get_guild_new_member_welcome() {
    let (client, transport) = setup_json(&json!({
        "guild_id": "100", "enabled": true,
        "welcome_message": {"author_ids": ["5"], "message": "Welcome!"},
        "new_member_actions": [{
            "channel_id": "7", "action_type": 1, "title": "Say hi", "description": "Post a hello",
            "emoji": {"id": null, "name": "👋", "animated": false}
        }],
        "resource_channels": [{"channel_id": "8", "title": "Rules", "description": "Read these"}]
    }));

    let welcome = client
        .guild_new_member_welcome(GUILD)
        .await
        .unwrap()
        .unwrap();

    single_request(&transport, "GET", "/guilds/100/new-member-welcome");
    assert_eq!(welcome.welcome_message.unwrap().message, "Welcome!");
    assert_eq!(
        welcome.new_member_actions[0].action_type,
        cordial_http::endpoints::guild_settings::NewMemberActionType::TALK
    );
    assert_eq!(welcome.resource_channels[0].title, "Rules");
}

#[tokio::test]
async fn get_guild_new_member_welcome_gives_none_for_204() {
    let (client, _transport) = setup_empty();

    let welcome = client.guild_new_member_welcome(GUILD).await.unwrap();

    assert!(welcome.is_none());
}

fn join_request_json(status: &str) -> serde_json::Value {
    json!({
        "id": "321", "created_at": "2026-01-01T00:00:00+00:00", "reviewed_at": null,
        "application_status": status, "rejection_reason": null, "guild_id": "100",
        "user_id": "53908099506183680", "user": user_json("53908099506183680"),
        "form_responses": [{"field_type": "TERMS", "label": "Rules", "values": ["Be nice"], "response": true}]
    })
}

#[tokio::test]
async fn get_guild_join_requests() {
    let (client, transport) = setup_json(&json!({
        "total": 1, "guild_join_requests": [join_request_json("SUBMITTED")]
    }));

    let list = client
        .guild_join_requests(GUILD)
        .status(JoinRequestStatus::Submitted)
        .limit(10)
        .after(GenericId::new_const(5))
        .await
        .unwrap();

    single_request(
        &transport,
        "GET",
        "/guilds/100/requests?status=SUBMITTED&limit=10&after=5",
    );
    assert_eq!(list.total, 1);
    assert_eq!(
        list.guild_join_requests[0].application_status,
        Some(JoinRequestStatus::Submitted)
    );
}

#[tokio::test]
async fn edit_guild_join_request() {
    let (client, transport) = setup_json(&join_request_json("REJECTED"));

    let request_data = client
        .edit_guild_join_request(
            GUILD,
            GenericId::new_const(321),
            JoinRequestStatus::Rejected,
        )
        .rejection_reason("Too new")
        .await
        .unwrap();

    let request = single_request(&transport, "PATCH", "/guilds/100/requests/321");
    assert_eq!(
        request.json(),
        json!({"action": "REJECTED", "rejection_reason": "Too new"})
    );
    assert_eq!(
        request_data.application_status,
        Some(JoinRequestStatus::Rejected)
    );
}

#[tokio::test]
async fn unknown_join_request_status_decodes() {
    let status: JoinRequestStatus = serde_json::from_value(json!("WAITLISTED")).unwrap();

    assert_eq!(status, JoinRequestStatus::Unknown);
}
