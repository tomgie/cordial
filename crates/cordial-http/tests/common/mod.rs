//! Helpers and response fixtures for the endpoint tests.

// Each test file uses a different part of this module.
#![allow(dead_code)]

use cordial_http::testing::{FakeResponse, FakeTransport, RecordedRequest};
use cordial_http::{Client, Token};
use cordial_model::id::GuildId;
use serde_json::{Value, json};

/// The base URL of the API.
pub(crate) const BASE: &str = "https://discord.com/api/v10";
/// The guild of the tests.
pub(crate) const GUILD: GuildId = GuildId::new_const(100);

/// Makes a client with a fake transport that gives one response.
pub(crate) fn setup(response: FakeResponse) -> (Client, FakeTransport) {
    let transport = FakeTransport::new();
    transport.push(response);
    let client = Client::builder()
        .token(Token::bot("abc"))
        .transport(transport.clone())
        .build();
    (client, transport)
}

/// Makes a client with a fake transport that gives a JSON response with status 200.
pub(crate) fn setup_json(body: &Value) -> (Client, FakeTransport) {
    setup(FakeResponse::json(200, body))
}

/// Makes a client with a fake transport that gives status 204.
pub(crate) fn setup_empty() -> (Client, FakeTransport) {
    setup(FakeResponse::empty(204))
}

/// Gives the one request that the transport received, and checks its method and path.
pub(crate) fn single_request(
    transport: &FakeTransport,
    method: &str,
    path: &str,
) -> RecordedRequest {
    let requests = transport.requests();
    assert_eq!(requests.len(), 1, "expected one request");
    let request = requests.into_iter().next().unwrap();
    assert_eq!(request.method, method);
    assert_eq!(request.url, format!("{BASE}{path}"));
    request
}

/// Checks the audit log reason header.
pub(crate) fn assert_reason(request: &RecordedRequest, reason: &str) {
    let encoded: String = percent_encode(reason);
    assert_eq!(request.header("x-audit-log-reason"), Some(encoded.as_str()));
}

fn percent_encode(text: &str) -> String {
    let mut encoded = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// A user.
pub(crate) fn user_json(id: &str) -> Value {
    json!({
        "id": id, "username": "Mason", "discriminator": "9999", "global_name": null,
        "avatar": "a_bab14f271d565501444b2ca3be944b25", "public_flags": 131_141
    })
}

/// A guild member, from the example in the docs.
pub(crate) fn member_json() -> Value {
    json!({
        "user": user_json("53908099506183680"),
        "nick": "NOT API SUPPORT",
        "avatar": null,
        "banner": null,
        "roles": ["41771983423143936"],
        "joined_at": "2015-04-26T06:26:56.936000+00:00",
        "deaf": false,
        "mute": false,
        "flags": 0,
        "pending": false,
        "communication_disabled_until": null
    })
}

/// A role.
pub(crate) fn role_json(id: &str, position: i32) -> Value {
    json!({
        "id": id, "name": "WE DEM BOYZZ!!!!!!", "color": 3_447_003,
        "colors": {"primary_color": 3_447_003, "secondary_color": null, "tertiary_color": null},
        "hoist": true, "icon": "cf3ced8600b777c9486c9a5c6e4a4e38", "unicode_emoji": null,
        "position": position, "permissions": "66321471", "managed": false, "mentionable": false,
        "flags": 0
    })
}

/// A text channel.
pub(crate) fn channel_json(id: &str) -> Value {
    json!({
        "id": id, "type": 0, "guild_id": "100", "position": 6,
        "permission_overwrites": [], "name": "general",
        "topic": "24/7 chat about how to gank Mike #2", "nsfw": true,
        "last_message_id": "155117677105512449", "rate_limit_per_user": 2,
        "parent_id": "399942396007890945"
    })
}

/// A guild, from the example of Get Guild in the docs.
pub(crate) fn guild_json() -> Value {
    json!({
        "id": "2909267986263572999",
        "name": "Mason's Test Server",
        "icon": "389030ec9db118cb5b85a732333b7c98",
        "description": null,
        "splash": "75610b05a0dd09ec2c3c7df9f6975ea0",
        "discovery_splash": null,
        "approximate_member_count": 2,
        "approximate_presence_count": 2,
        "features": ["INVITE_SPLASH", "VANITY_URL", "COMMERCE", "BANNER", "NEWS", "VERIFIED", "VIP_REGIONS"],
        "emojis": [{
            "name": "ultrafastparrot", "roles": [], "id": "393564762228785161",
            "require_colons": true, "managed": false, "animated": true, "available": true
        }],
        "banner": "5c3cb8d1bc159937fffe7e641ec96ca7",
        "owner_id": "53908232506183680",
        "application_id": null,
        "region": null,
        "afk_channel_id": null,
        "afk_timeout": 300,
        "system_channel_id": null,
        "widget_enabled": true,
        "widget_channel_id": "639513352485470208",
        "verification_level": 0,
        "roles": [{
            "id": "2909267986263572999", "name": "@everyone", "permissions": "49794752",
            "position": 0, "color": 0,
            "colors": {"primary_color": 0, "secondary_color": null, "tertiary_color": null},
            "hoist": false, "managed": false, "mentionable": false
        }],
        "default_message_notifications": 1,
        "mfa_level": 0,
        "explicit_content_filter": 0,
        "max_presences": null,
        "max_members": 250_000,
        "max_video_channel_users": 25,
        "vanity_url_code": "no",
        "premium_tier": 0,
        "premium_subscription_count": 0,
        "system_channel_flags": 0,
        "preferred_locale": "en-US",
        "rules_channel_id": null,
        "public_updates_channel_id": null,
        "safety_alerts_channel_id": null
    })
}

/// A scheduled event.
pub(crate) fn scheduled_event_json() -> Value {
    json!({
        "id": "900", "guild_id": "100", "channel_id": null, "creator_id": "53908099506183680",
        "name": "Game night", "description": "Bring snacks",
        "scheduled_start_time": "2026-12-01T20:00:00+00:00",
        "scheduled_end_time": "2026-12-01T23:00:00+00:00",
        "privacy_level": 2, "status": 1, "entity_type": 3, "entity_id": null,
        "entity_metadata": {"location": "Main hall"}, "user_count": 12, "image": null,
        "recurrence_rule": null
    })
}
