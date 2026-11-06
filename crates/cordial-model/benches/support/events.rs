//! Generated gateway event data for the benchmarks.
//!
//! The benchmarks of `cordial-model`, `cordial-gateway`, and `cordial-cache` include this file
//! with `#[path]`, so that all of them measure the same data.

// Each benchmark uses only some of the events.
#![allow(dead_code)]

use serde_json::{Value, json};

/// The ID of the generated guild.
pub(crate) const GUILD_ID: u64 = 81_384_788_765_712_384;

/// The size of a generated guild.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GuildSize {
    /// The number of members.
    pub(crate) members: u64,
    /// The number of channels.
    pub(crate) channels: u64,
    /// The number of roles.
    pub(crate) roles: u64,
}

/// The large guild of the benchmarks: 1,000 members, 200 channels, and 100 roles.
pub(crate) const LARGE_GUILD: GuildSize = GuildSize {
    members: 1_000,
    channels: 200,
    roles: 100,
};

fn user(id: u64) -> Value {
    json!({
        "id": id.to_string(),
        "username": format!("user{id}"),
        "discriminator": "0",
        "global_name": format!("User {id}"),
        "avatar": "a_f03401914fb4f3caa9037578ab980920",
        "public_flags": 64
    })
}

fn member(user_id: u64, roles: &[u64]) -> Value {
    let roles: Vec<String> = roles.iter().map(u64::to_string).collect();
    json!({
        "user": user(user_id),
        "nick": null,
        "avatar": null,
        "roles": roles,
        "joined_at": "2026-01-01T00:00:00.000000+00:00",
        "premium_since": null,
        "deaf": false,
        "mute": false,
        "pending": false,
        "flags": 0
    })
}

fn role(id: u64, position: u64) -> Value {
    json!({
        "id": id.to_string(),
        "name": format!("role {position}"),
        "color": 3_447_003,
        "colors": {"primary_color": 3_447_003, "secondary_color": null, "tertiary_color": null},
        "hoist": position.is_multiple_of(10),
        "icon": null,
        "unicode_emoji": null,
        "position": position,
        "permissions": "2248473465835073",
        "managed": false,
        "mentionable": true,
        "flags": 0
    })
}

fn channel(id: u64, position: u64, parent: Option<u64>) -> Value {
    json!({
        "id": id.to_string(),
        "type": if parent.is_some() { 0 } else { 4 },
        "guild_id": GUILD_ID.to_string(),
        "name": format!("channel-{position}"),
        "position": position,
        "parent_id": parent.map(|parent| parent.to_string()),
        "topic": "A channel for the benchmark",
        "nsfw": false,
        "rate_limit_per_user": 0,
        "last_message_id": "1300000000000000000",
        "permission_overwrites": [
            {"id": GUILD_ID.to_string(), "type": 0, "allow": "0", "deny": "1024"},
            {"id": (GUILD_ID + 1).to_string(), "type": 0, "allow": "1024", "deny": "0"}
        ],
        "flags": 0
    })
}

/// Gives the data of a GUILD_CREATE event for a guild of a size.
#[must_use]
pub(crate) fn guild_create(guild_id: u64, size: GuildSize) -> Value {
    let roles: Vec<Value> = (0..size.roles)
        .map(|index| {
            // The first role is @everyone: its ID is the guild ID.
            let id = if index == 0 {
                guild_id
            } else {
                guild_id + index
            };
            role(id, index)
        })
        .collect();
    let members: Vec<Value> = (0..size.members)
        .map(|index| {
            let role_ids = [guild_id + 1 + index % size.roles.max(2).saturating_sub(1)];
            member(1_000_000 + index, &role_ids)
        })
        .collect();
    let channels: Vec<Value> = (0..size.channels)
        .map(|index| {
            let id = guild_id + 100_000 + index;
            // One category for each 20 channels.
            let parent = (index % 20 != 0).then(|| guild_id + 100_000 + index / 20 * 20);
            channel(id, index, parent)
        })
        .collect();

    json!({
        "id": guild_id.to_string(),
        "name": "Benchmark guild",
        "icon": null, "splash": null, "discovery_splash": null,
        "owner_id": "1000000",
        "afk_channel_id": null, "afk_timeout": 300, "verification_level": 1,
        "default_message_notifications": 1, "explicit_content_filter": 2, "mfa_level": 0,
        "features": ["COMMUNITY", "NEWS"], "emojis": [], "stickers": [],
        "application_id": null, "system_channel_id": null, "system_channel_flags": 0,
        "rules_channel_id": null, "vanity_url_code": null, "description": null,
        "banner": null, "premium_tier": 2, "preferred_locale": "en-US",
        "public_updates_channel_id": null, "nsfw_level": 0,
        "premium_progress_bar_enabled": false, "safety_alerts_channel_id": null,
        "joined_at": "2026-01-01T00:00:00.000000+00:00", "large": true,
        "member_count": size.members,
        "roles": roles,
        "members": members,
        "channels": channels,
        "threads": [], "voice_states": [], "presences": [],
        "stage_instances": [], "guild_scheduled_events": [], "soundboard_sounds": []
    })
}

/// Gives the data of a MESSAGE_CREATE event in a guild channel.
#[must_use]
pub(crate) fn message_create(message_id: u64) -> Value {
    json!({
        "id": message_id.to_string(),
        "channel_id": (GUILD_ID + 100_001).to_string(),
        "guild_id": GUILD_ID.to_string(),
        "author": user(1_000_001),
        "member": {
            "roles": [(GUILD_ID + 1).to_string()],
            "joined_at": "2026-01-01T00:00:00.000000+00:00",
            "deaf": false, "mute": false, "flags": 0
        },
        "content": "Hello <@1000002>, the release is out. See https://example.com/notes for the notes.",
        "timestamp": "2026-11-05T12:00:00.000000+00:00",
        "edited_timestamp": null,
        "tts": false,
        "mention_everyone": false,
        "mentions": [user(1_000_002)],
        "mention_roles": [],
        "attachments": [],
        "embeds": [{
            "type": "rich",
            "title": "Release notes",
            "description": "The new version has many changes.",
            "url": "https://example.com/notes",
            "color": 3_447_003,
            "fields": [
                {"name": "Version", "value": "1.2.0", "inline": true},
                {"name": "Date", "value": "2026-11-05", "inline": true}
            ]
        }],
        "pinned": false,
        "type": 0,
        "flags": 0,
        "nonce": "1300000000000000000"
    })
}

/// Gives the data of a PRESENCE_UPDATE event.
#[must_use]
pub(crate) fn presence_update() -> Value {
    json!({
        "user": {"id": "1000001"},
        "guild_id": GUILD_ID.to_string(),
        "status": "online",
        "activities": [{
            "name": "Rocket League",
            "type": 0,
            "created_at": 1_762_000_000_000_u64,
            "application_id": "379286085710381999",
            "state": "In a match",
            "details": "Ranked Duos: 2-1",
            "timestamps": {"start": 1_762_000_000_000_u64},
            "assets": {
                "large_image": "351371005538729000",
                "large_text": "DFH Stadium",
                "small_image": "351371005538729111",
                "small_text": "Silver III"
            }
        }],
        "client_status": {"desktop": "online"}
    })
}

/// Gives the data of an INTERACTION_CREATE event: a slash command with options.
#[must_use]
pub(crate) fn interaction_create() -> Value {
    json!({
        "id": "867794291820986368",
        "application_id": "775799577604522054",
        "type": 2,
        "data": {
            "id": "866818195033292850",
            "name": "moderate",
            "type": 1,
            "options": [
                {"name": "user", "type": 6, "value": "1000002"},
                {"name": "reason", "type": 3, "value": "Spam in the general channel"},
                {"name": "days", "type": 4, "value": 7}
            ],
            "resolved": {
                "users": {"1000002": user(1_000_002)},
                "members": {"1000002": {
                    "roles": [], "joined_at": "2026-01-01T00:00:00.000000+00:00",
                    "permissions": "246997699136", "flags": 0
                }}
            }
        },
        "guild_id": GUILD_ID.to_string(),
        "channel_id": (GUILD_ID + 100_001).to_string(),
        "channel": {"id": (GUILD_ID + 100_001).to_string(), "type": 0, "guild_id": GUILD_ID.to_string(), "name": "channel-1"},
        "member": member(1_000_001, &[GUILD_ID + 1]),
        "app_permissions": "442368",
        "token": "UNIQUE_TOKEN",
        "version": 1,
        "locale": "en-US",
        "guild_locale": "en-US",
        "entitlements": [],
        "authorizing_integration_owners": {"0": GUILD_ID.to_string()},
        "context": 0,
        "attachment_size_limit": 10_485_760
    })
}
