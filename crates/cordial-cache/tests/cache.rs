//! Tests of the cache with event sequences.
// The large `json!` literals need more macro recursion.
#![recursion_limit = "512"]

use cordial_cache::{Cache, PermissionError, Previous, ResourceTypes};
use cordial_model::Permissions;
use cordial_model::gateway::event::Event;
use cordial_model::id::{ChannelId, GuildId, MessageId, RoleId, UserId};
use serde_json::{Value, json};

const GUILD: GuildId = GuildId::new_const(100);
const OWNER: UserId = UserId::new_const(1);
const MEMBER: UserId = UserId::new_const(2);
const TEXT: ChannelId = ChannelId::new_const(10);
const SECRET: ChannelId = ChannelId::new_const(11);
const THREAD: ChannelId = ChannelId::new_const(12);
const HIDDEN: ChannelId = ChannelId::new_const(13);
const MODERATOR: RoleId = RoleId::new_const(50);

fn event(name: &str, data: &Value) -> Event {
    let raw = serde_json::value::RawValue::from_string(data.to_string()).unwrap();
    Event::decode(name, &raw).unwrap()
}

fn user(id: u64) -> Value {
    json!({"id": id.to_string(), "username": format!("user{id}"), "discriminator": "0", "avatar": null})
}

fn member(id: u64, roles: &[u64]) -> Value {
    let roles: Vec<String> = roles.iter().map(u64::to_string).collect();
    json!({"user": user(id), "roles": roles, "joined_at": "2026-01-01T00:00:00+00:00", "deaf": false, "mute": false, "flags": 0})
}

fn role(id: u64, permissions: Permissions) -> Value {
    json!({"id": id.to_string(), "name": "role", "color": 0, "hoist": false, "position": 1,
           "permissions": permissions.bits().to_string(), "managed": false, "mentionable": false, "flags": 0})
}

fn guild_create() -> Value {
    let everyone =
        Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::READ_MESSAGE_HISTORY;
    json!({
        "id": "100", "name": "Test", "icon": null, "splash": null, "discovery_splash": null,
        "owner_id": "1", "afk_channel_id": null, "afk_timeout": 300, "verification_level": 0,
        "default_message_notifications": 0, "explicit_content_filter": 0, "mfa_level": 0,
        "features": [], "emojis": [], "application_id": null, "system_channel_id": null,
        "system_channel_flags": 0, "rules_channel_id": null, "vanity_url_code": null, "description": null,
        "banner": null, "premium_tier": 0, "preferred_locale": "en-US", "public_updates_channel_id": null,
        "nsfw_level": 0, "premium_progress_bar_enabled": false, "safety_alerts_channel_id": null,
        "joined_at": "2026-01-01T00:00:00+00:00", "large": false, "member_count": 2,
        "roles": [role(100, everyone), role(50, Permissions::MANAGE_MESSAGES)],
        "members": [member(1, &[]), member(2, &[50])],
        "channels": [
            {"id": "10", "type": 0, "name": "general", "position": 0, "permission_overwrites": []},
            {"id": "11", "type": 0, "name": "secret", "position": 1, "permission_overwrites": [
                {"id": "100", "type": 0, "allow": "0", "deny": Permissions::VIEW_CHANNEL.bits().to_string()},
                {"id": "50", "type": 0, "allow": Permissions::VIEW_CHANNEL.bits().to_string(), "deny": "0"}
            ]},
            {"id": "13", "type": 0, "name": "___hidden___", "position": 2, "flags": 131072,
             "permission_overwrites": [{"id": "100", "type": 0, "allow": "0", "deny": "1024"}]}
        ],
        "threads": [{"id": "12", "type": 11, "name": "thread", "parent_id": "11",
                     "thread_metadata": {"archived": false, "auto_archive_duration": 60, "archive_timestamp": "2026-01-01T00:00:00+00:00", "locked": false}}],
        "voice_states": [], "presences": []
    })
}

fn loaded_cache(resources: ResourceTypes) -> Cache {
    let cache = Cache::new(resources);
    cache.update(&event("READY", &json!({
        "v": 10, "user": user(99), "guilds": [{"id": "100", "unavailable": true}],
        "session_id": "s", "resume_gateway_url": "wss://x", "application": {"id": "99", "flags": 0}
    })));
    assert!(cache.is_guild_unavailable(GUILD));
    cache.update(&event("GUILD_CREATE", &guild_create()));
    cache
}

#[test]
fn guild_create_fills_the_cache() {
    let cache = loaded_cache(ResourceTypes::all());

    let guild = cache.guild(GUILD).unwrap();
    assert!(!cache.is_guild_unavailable(GUILD));
    assert!(guild.roles.is_empty(), "the cached guild has no role list");
    assert_eq!(cache.guild_roles(GUILD).len(), 2);
    assert_eq!(cache.guild_channels(GUILD).len(), 4);
    assert_eq!(cache.guild_members(GUILD).len(), 2);
    assert_eq!(cache.user(MEMBER).unwrap().username, "user2");
    assert_eq!(cache.current_user().unwrap().id.get(), 99);
    assert_eq!(cache.channel(THREAD).unwrap().guild_id, Some(GUILD));
}

#[test]
fn member_update_gives_the_previous_member() {
    let cache = loaded_cache(ResourceTypes::all());

    let previous = cache.update(&event("GUILD_MEMBER_UPDATE", &json!({
        "guild_id": "100", "roles": [], "user": user(2), "nick": "new", "avatar": null, "banner": null,
        "joined_at": "2026-01-01T00:00:00+00:00"
    })));

    let Some(Previous::Member(old)) = previous else {
        panic!("expected the previous member");
    };
    assert_eq!(old.roles, vec![MODERATOR]);
    let current = cache.member(GUILD, MEMBER).unwrap();
    assert!(current.roles.is_empty());
    assert_eq!(current.nick.as_deref(), Some("new"));
}

#[test]
fn member_remove_removes_the_user() {
    let cache = loaded_cache(ResourceTypes::all());

    cache.update(&event(
        "GUILD_MEMBER_REMOVE",
        &json!({"guild_id": "100", "user": user(2)}),
    ));

    assert!(cache.member(GUILD, MEMBER).is_none());
    assert!(cache.user(MEMBER).is_none());
    assert_eq!(cache.guild(GUILD).unwrap().member_count, Some(1));
}

#[test]
fn guild_delete_removes_everything() {
    let cache = loaded_cache(ResourceTypes::all());

    cache.update(&event("GUILD_DELETE", &json!({"id": "100"})));

    assert!(cache.guild(GUILD).is_none());
    assert!(!cache.is_guild_unavailable(GUILD));
    assert!(cache.channel(TEXT).is_none());
    assert!(cache.role(MODERATOR).is_none());
    assert!(cache.member(GUILD, MEMBER).is_none());
    assert_eq!(cache.stats().users, 1, "only the current user stays");
}

#[test]
fn computes_permissions() {
    let cache = loaded_cache(ResourceTypes::all());

    assert_eq!(
        cache.guild_permissions(GUILD, OWNER).unwrap(),
        Permissions::all()
    );
    let member = cache.guild_permissions(GUILD, MEMBER).unwrap();
    assert!(member.contains(Permissions::MANAGE_MESSAGES | Permissions::SEND_MESSAGES));

    // The secret channel denies VIEW_CHANNEL to everyone but allows it to the moderator role.
    assert!(
        cache
            .channel_permissions(SECRET, MEMBER)
            .unwrap()
            .contains(Permissions::VIEW_CHANNEL)
    );

    // Threads use the overwrites of the parent, and need SEND_MESSAGES_IN_THREADS to send.
    let thread = cache.channel_permissions(THREAD, MEMBER).unwrap();
    assert!(thread.contains(Permissions::VIEW_CHANNEL));
    assert!(!thread.contains(Permissions::SEND_MESSAGES));

    assert_eq!(
        cache.channel_permissions(HIDDEN, MEMBER),
        Err(PermissionError::Obfuscated(HIDDEN))
    );
}

#[test]
fn removing_the_role_changes_permissions() {
    let cache = loaded_cache(ResourceTypes::all());

    cache.update(&event(
        "GUILD_ROLE_DELETE",
        &json!({"guild_id": "100", "role_id": "50"}),
    ));

    let secret = cache.channel_permissions(SECRET, MEMBER).unwrap();
    assert!(!secret.contains(Permissions::VIEW_CHANNEL));
}

#[test]
fn keeps_recent_messages_only() {
    let cache = Cache::with_message_capacity(ResourceTypes::MESSAGE, 2);
    for id in 1..=3_u64 {
        cache.update(&event("MESSAGE_CREATE", &json!({
            "id": id.to_string(), "channel_id": "10", "author": user(2), "content": format!("m{id}"),
            "timestamp": "2026-01-01T00:00:00+00:00", "edited_timestamp": null, "tts": false,
            "mention_everyone": false, "mentions": [], "mention_roles": [], "attachments": [],
            "embeds": [], "pinned": false, "type": 0
        })));
    }

    let messages = cache.channel_messages(TEXT);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].content, "m2");

    let deleted = cache.update(&event(
        "MESSAGE_DELETE",
        &json!({"id": "3", "channel_id": "10"}),
    ));
    assert!(matches!(deleted, Some(Previous::Message(message)) if message.content == "m3"));
    assert!(cache.message(TEXT, MessageId::new_const(3)).is_none());
}

#[test]
fn voice_state_leave_removes_the_state() {
    let cache = loaded_cache(ResourceTypes::all());
    let state = |channel: Value| {
        json!({"guild_id": "100", "channel_id": channel, "user_id": "2", "session_id": "x",
        "deaf": false, "mute": false, "self_deaf": false, "self_mute": false, "self_video": false,
        "suppress": false, "request_to_speak_timestamp": null})
    };

    cache.update(&event("VOICE_STATE_UPDATE", &state(json!("10"))));
    assert_eq!(cache.voice_channel_states(TEXT).len(), 1);

    cache.update(&event("VOICE_STATE_UPDATE", &state(Value::Null)));
    assert!(cache.voice_state(GUILD, MEMBER).is_none());
}

#[test]
fn unselected_resources_stay_empty() {
    let cache = loaded_cache(ResourceTypes::GUILD);

    assert!(cache.guild(GUILD).is_some());
    assert!(cache.channel(TEXT).is_none());
    assert!(cache.member(GUILD, MEMBER).is_none());
}

#[test]
fn resources_give_events_and_intents() {
    use cordial_model::gateway::{EventType, Intents};

    let resources = ResourceTypes::MEMBER | ResourceTypes::PRESENCE;

    assert!(resources.event_types().contains(EventType::GuildMemberAdd));
    assert!(
        resources
            .intents()
            .contains(Intents::GUILD_MEMBERS | Intents::GUILD_PRESENCES)
    );
}

fn message_create(id: u64) -> Event {
    event(
        "MESSAGE_CREATE",
        &json!({
            "id": id.to_string(), "channel_id": "10", "author": user(2), "content": "text",
            "timestamp": "2026-01-01T00:00:00+00:00", "edited_timestamp": null, "tts": false,
            "mention_everyone": false, "mentions": [], "mention_roles": [], "attachments": [],
            "embeds": [], "pinned": false, "type": 0
        }),
    )
}

fn reaction(name: &str, user_id: u64, emoji: &str, burst: bool) -> Event {
    event(
        name,
        &json!({"user_id": user_id.to_string(), "channel_id": "10", "message_id": "1",
                "emoji": {"id": null, "name": emoji}, "burst": burst, "type": u8::from(burst)}),
    )
}

#[test]
fn reactions_change_cached_messages() {
    let cache = loaded_cache(ResourceTypes::all());
    cache.update(&message_create(1));
    let before = cache.message(TEXT, MessageId::new_const(1)).unwrap();

    cache.update(&reaction("MESSAGE_REACTION_ADD", 2, "👍", false));
    cache.update(&reaction("MESSAGE_REACTION_ADD", 99, "👍", false));
    cache.update(&reaction("MESSAGE_REACTION_ADD", 2, "🔥", true));

    let message = cache.message(TEXT, MessageId::new_const(1)).unwrap();
    let reactions = message.reactions.as_ref().unwrap();
    assert_eq!(reactions.len(), 2);
    assert_eq!(reactions[0].count, 2);
    assert!(reactions[0].me, "user 99 is the current user");
    assert_eq!(reactions[1].count_details.burst, 1);
    assert!(before.reactions.is_none(), "an old Arc keeps the old value");

    cache.update(&reaction("MESSAGE_REACTION_REMOVE", 99, "👍", false));
    cache.update(&event(
        "MESSAGE_REACTION_REMOVE_EMOJI",
        &json!({"channel_id": "10", "message_id": "1", "emoji": {"id": null, "name": "🔥"}}),
    ));

    let message = cache.message(TEXT, MessageId::new_const(1)).unwrap();
    let reactions = message.reactions.as_ref().unwrap();
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0].count, 1);
    assert!(!reactions[0].me);

    cache.update(&event(
        "MESSAGE_REACTION_REMOVE_ALL",
        &json!({"channel_id": "10", "message_id": "1"}),
    ));
    let message = cache.message(TEXT, MessageId::new_const(1)).unwrap();
    assert!(message.reactions.is_none());
}

/// Events can come for objects that the cache does not know. The cache must ignore them.
#[test]
fn events_for_unknown_objects_do_not_panic() {
    let cache = Cache::new(ResourceTypes::all());
    let events = [
        event(
            "GUILD_MEMBER_UPDATE",
            &json!({"guild_id": "7", "roles": [], "user": user(3), "joined_at": null}),
        ),
        event(
            "GUILD_MEMBER_REMOVE",
            &json!({"guild_id": "7", "user": user(3)}),
        ),
        event(
            "GUILD_ROLE_DELETE",
            &json!({"guild_id": "7", "role_id": "8"}),
        ),
        event(
            "CHANNEL_DELETE",
            &json!({"id": "9", "type": 0, "guild_id": "7"}),
        ),
        event(
            "THREAD_DELETE",
            &json!({"id": "9", "type": 11, "guild_id": "7", "parent_id": "1"}),
        ),
        event("MESSAGE_DELETE", &json!({"id": "1", "channel_id": "9"})),
        event(
            "MESSAGE_DELETE_BULK",
            &json!({"ids": ["1", "2"], "channel_id": "9"}),
        ),
        reaction("MESSAGE_REACTION_ADD", 3, "👍", false),
        event("GUILD_DELETE", &json!({"id": "7"})),
        event(
            "STAGE_INSTANCE_DELETE",
            &json!({"id": "5", "guild_id": "7", "channel_id": "9",
            "topic": "t", "privacy_level": 2, "discoverable_disabled": false, "guild_scheduled_event_id": null}),
        ),
        event(
            "GUILD_SOUNDBOARD_SOUND_DELETE",
            &json!({"sound_id": "5", "guild_id": "7"}),
        ),
    ];

    for event in &events {
        assert!(cache.update(event).is_none(), "{event:?}");
    }

    let stats = cache.stats();
    assert_eq!(stats.guilds, 0);
    assert_eq!(stats.members, 0);
    assert_eq!(stats.messages, 0);
    assert_eq!(stats.users, 0, "users of uncached guilds leak");
}

/// Prints the stack size of the cached types. Run with `--nocapture` to see the values.
#[expect(clippy::print_stdout, reason = "the test reports the sizes")]
#[test]
fn cached_type_sizes() {
    use cordial_cache::{CachedGuild, CachedMember, CachedRole};
    use cordial_model::channel::Channel;
    use cordial_model::user::User;
    use std::mem::size_of;

    let sizes = [
        ("CachedGuild", size_of::<CachedGuild>()),
        ("CachedMember", size_of::<CachedMember>()),
        ("CachedRole", size_of::<CachedRole>()),
        ("Channel", size_of::<Channel>()),
        ("User", size_of::<User>()),
    ];
    for (name, size) in sizes {
        println!("{name}: {size} bytes");
    }

    assert!(
        size_of::<CachedMember>() < 512,
        "members are the most numerous objects"
    );
}

#[test]
fn outage_keeps_the_data_and_the_next_create_replaces_channels_and_roles() {
    let cache = loaded_cache(ResourceTypes::all());

    cache.update(&event(
        "GUILD_DELETE",
        &json!({"id": "100", "unavailable": true}),
    ));
    assert!(cache.is_guild_unavailable(GUILD));
    assert!(
        cache.member(GUILD, MEMBER).is_some(),
        "an outage keeps the members"
    );

    // During the outage, the secret channel and the moderator role were deleted.
    let mut guild = guild_create();
    guild["channels"]
        .as_array_mut()
        .unwrap()
        .retain(|channel| channel["id"] != "11");
    guild["threads"] = json!([]);
    guild["roles"]
        .as_array_mut()
        .unwrap()
        .retain(|role| role["id"] != "50");
    cache.update(&event("GUILD_CREATE", &guild));

    assert!(!cache.is_guild_unavailable(GUILD));
    assert!(cache.channel(SECRET).is_none());
    assert!(cache.channel(THREAD).is_none());
    assert!(cache.role(MODERATOR).is_none());
    assert!(cache.channel(TEXT).is_some());
}

#[test]
fn channel_delete_removes_its_threads() {
    let cache = loaded_cache(ResourceTypes::all());

    cache.update(&event(
        "CHANNEL_DELETE",
        &json!({"id": "11", "type": 0, "guild_id": "100", "name": "secret", "position": 1}),
    ));

    assert!(cache.channel(SECRET).is_none());
    assert!(
        cache.channel(THREAD).is_none(),
        "the thread of the channel goes too"
    );
    assert!(cache.channel(TEXT).is_some());
}

#[test]
fn thread_list_sync_removes_threads_that_are_not_active() {
    let cache = loaded_cache(ResourceTypes::all());

    cache.update(&event(
        "THREAD_LIST_SYNC",
        &json!({"guild_id": "100", "channel_ids": ["11"], "threads": [], "members": []}),
    ));

    assert!(cache.channel(THREAD).is_none());
    assert!(cache.channel(SECRET).is_some());
}

#[test]
fn ready_removes_guilds_that_the_bot_left_while_offline() {
    let cache = loaded_cache(ResourceTypes::all());

    cache.update(&event(
        "READY",
        &json!({"v": 10, "user": user(99), "guilds": [], "session_id": "s2",
                "resume_gateway_url": "wss://x", "application": {"id": "99", "flags": 0}}),
    ));

    assert!(cache.guild(GUILD).is_none());
    assert!(cache.channel(TEXT).is_none());
    assert_eq!(cache.stats().members, 0);
}

#[test]
fn ready_of_another_shard_keeps_the_guild() {
    let cache = loaded_cache(ResourceTypes::all());

    // Guild 100 is on shard 0 of 2: (100 >> 22) % 2 == 0.
    cache.update(&event(
        "READY",
        &json!({"v": 10, "user": user(99), "guilds": [], "session_id": "s2", "shard": [1, 2],
                "resume_gateway_url": "wss://x", "application": {"id": "99", "flags": 0}}),
    ));

    assert!(cache.guild(GUILD).is_some());
}

#[test]
fn member_remove_removes_the_voice_state() {
    let cache = loaded_cache(ResourceTypes::all());
    cache.update(&event(
        "VOICE_STATE_UPDATE",
        &json!({"guild_id": "100", "channel_id": "10", "user_id": "2", "session_id": "x",
                "deaf": false, "mute": false, "self_deaf": false, "self_mute": false,
                "self_video": false, "suppress": false, "request_to_speak_timestamp": null}),
    ));

    cache.update(&event(
        "GUILD_MEMBER_REMOVE",
        &json!({"guild_id": "100", "user": user(2)}),
    ));

    assert!(cache.voice_state(GUILD, MEMBER).is_none());
}

#[test]
fn message_buffers_without_the_channel_resource_go_with_the_guild() {
    let cache = Cache::with_message_capacity(ResourceTypes::GUILD | ResourceTypes::MESSAGE, 5);
    cache.update(&event("GUILD_CREATE", &guild_create()));
    let mut message = json!({
        "id": "1", "channel_id": "10", "guild_id": "100", "author": user(2), "content": "m",
        "timestamp": "2026-01-01T00:00:00+00:00", "edited_timestamp": null, "tts": false,
        "mention_everyone": false, "mentions": [], "mention_roles": [], "attachments": [],
        "embeds": [], "pinned": false, "type": 0
    });
    cache.update(&event("MESSAGE_CREATE", &message));
    message["id"] = json!("2");
    message["channel_id"] = json!("777");
    message["guild_id"] = Value::Null;
    cache.update(&event("MESSAGE_CREATE", &message));
    assert_eq!(cache.stats().messages, 2);

    cache.update(&event("GUILD_DELETE", &json!({"id": "100"})));

    assert_eq!(cache.stats().messages, 1, "only the DM buffer stays");
    assert!(
        ResourceTypes::MESSAGE
            .event_types()
            .contains(cordial_model::gateway::EventType::ChannelDelete)
    );
}
