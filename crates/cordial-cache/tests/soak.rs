//! A synthetic soak test: many guilds and a long random event stream.
//!
//! The test keeps a shadow model of what the cache must hold, and compares the counts after
//! each phase. At the end it deletes all guilds: all counts must go back to zero, so a leak
//! fails the test. `cargo test -p cordial-cache --test soak -- --ignored` runs the large
//! version (1000 guilds).

use std::collections::{BTreeMap, BTreeSet};

use cordial_cache::{Cache, ResourceTypes};
use cordial_model::gateway::event::Event;
use serde_json::{Value, json};

const MESSAGE_CAPACITY: usize = 5;
const CURRENT_USER: u64 = 1;

/// A small deterministic random generator (xorshift64*), so that a failure repeats.
struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, limit: u64) -> u64 {
        self.next() % limit
    }

    fn pick<T: Copy>(&mut self, items: &BTreeSet<T>) -> Option<T> {
        if items.is_empty() {
            return None;
        }
        let index = usize::try_from(self.below(items.len() as u64)).unwrap();
        items.iter().nth(index).copied()
    }
}

/// What one guild holds in the shadow model.
#[derive(Default)]
struct GuildModel {
    members: BTreeSet<u64>,
    channels: BTreeSet<u64>,
    threads: BTreeMap<u64, u64>,
    roles: BTreeSet<u64>,
    /// Channels and threads with messages: the count for each.
    messages: BTreeMap<u64, usize>,
}

struct Soak {
    cache: Cache,
    random: Random,
    guilds: BTreeMap<u64, GuildModel>,
    next_id: u64,
    user_pool: u64,
}

fn decode(name: &str, data: &Value) -> Event {
    let raw = serde_json::value::RawValue::from_string(data.to_string()).unwrap();
    Event::decode(name, &raw).unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn user(id: u64) -> Value {
    json!({"id": id.to_string(), "username": format!("u{id}"), "discriminator": "0", "avatar": null})
}

fn member(id: u64, roles: &[u64]) -> Value {
    let roles: Vec<String> = roles.iter().map(u64::to_string).collect();
    json!({"user": user(id), "roles": roles, "joined_at": "2026-01-01T00:00:00+00:00",
           "deaf": false, "mute": false, "flags": 0})
}

fn role(id: u64) -> Value {
    json!({"id": id.to_string(), "name": "r", "color": 0, "hoist": false, "position": 1,
           "permissions": "1024", "managed": false, "mentionable": false, "flags": 0})
}

fn channel(id: u64, guild: u64) -> Value {
    json!({"id": id.to_string(), "type": 0, "guild_id": guild.to_string(), "name": "c",
           "position": 0, "permission_overwrites": []})
}

fn thread(id: u64, guild: u64, parent: u64) -> Value {
    json!({"id": id.to_string(), "type": 11, "guild_id": guild.to_string(), "name": "t",
           "parent_id": parent.to_string(),
           "thread_metadata": {"archived": false, "auto_archive_duration": 60,
                               "archive_timestamp": "2026-01-01T00:00:00+00:00", "locked": false}})
}

impl Soak {
    fn new(seed: u64, user_pool: u64) -> Self {
        let cache = Cache::with_message_capacity(ResourceTypes::all(), MESSAGE_CAPACITY);
        cache.update(&decode(
            "READY",
            &json!({"v": 10, "user": user(CURRENT_USER), "guilds": [], "session_id": "s",
                    "resume_gateway_url": "wss://x", "application": {"id": "1", "flags": 0}}),
        ));
        Self {
            cache,
            random: Random(seed),
            guilds: BTreeMap::new(),
            next_id: 1_000_000,
            user_pool,
        }
    }

    fn id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn random_user(&mut self) -> u64 {
        // Users repeat across guilds, so that the shared user count is tested.
        10 + self.random.below(self.user_pool)
    }

    fn send(&self, name: &str, data: &Value) {
        self.cache.update(&decode(name, data));
    }

    fn create_guild(&mut self, members: usize, channels: usize, roles: usize) {
        let guild_id = self.id();
        let mut model = GuildModel::default();
        // The @everyone role has the guild ID.
        model.roles.insert(guild_id);
        for _ in 1..roles {
            let role_id = self.id();
            model.roles.insert(role_id);
        }
        for _ in 0..channels {
            let channel_id = self.id();
            model.channels.insert(channel_id);
        }
        for _ in 0..members {
            let user_id = self.random_user();
            model.members.insert(user_id);
        }

        let roles: Vec<Value> = model.roles.iter().map(|id| role(*id)).collect();
        let channels: Vec<Value> = model
            .channels
            .iter()
            .map(|id| channel(*id, guild_id))
            .collect();
        let members: Vec<Value> = model.members.iter().map(|id| member(*id, &[])).collect();
        let mut guild = json!({
            "id": guild_id.to_string(), "name": "g", "icon": null, "splash": null,
            "discovery_splash": null, "owner_id": "10", "afk_channel_id": null, "afk_timeout": 300,
            "verification_level": 0, "default_message_notifications": 0,
            "explicit_content_filter": 0, "mfa_level": 0, "features": [], "emojis": [],
            "application_id": null, "system_channel_id": null, "system_channel_flags": 0,
            "rules_channel_id": null, "vanity_url_code": null, "description": null, "banner": null,
            "premium_tier": 0, "preferred_locale": "en-US", "public_updates_channel_id": null,
            "nsfw_level": 0, "premium_progress_bar_enabled": false, "safety_alerts_channel_id": null,
            "joined_at": "2026-01-01T00:00:00+00:00", "large": false, "member_count": members.len(),
            "threads": [], "voice_states": [], "presences": []
        });
        guild["roles"] = Value::Array(roles);
        guild["channels"] = Value::Array(channels);
        guild["members"] = Value::Array(members);

        self.send("GUILD_CREATE", &guild);
        self.guilds.insert(guild_id, model);
    }

    fn delete_guild(&mut self, guild_id: u64) {
        // A removal, not an outage: an outage keeps the data (see `outage_keeps_the_data`).
        self.send("GUILD_DELETE", &json!({"id": guild_id.to_string()}));
        self.guilds.remove(&guild_id);
    }

    /// Sends one random event for a random guild.
    fn step(&mut self) {
        let Some(guild_id) = self.random.pick(&self.guilds.keys().copied().collect()) else {
            return;
        };
        let guild = guild_id.to_string();

        match self.random.below(12) {
            0 => {
                let user_id = self.random_user();
                self.send("GUILD_MEMBER_ADD", &{
                    let mut data = member(user_id, &[]);
                    data["guild_id"] = json!(guild);
                    data
                });
                self.guilds
                    .get_mut(&guild_id)
                    .unwrap()
                    .members
                    .insert(user_id);
            }
            1 => {
                let members = self.guilds[&guild_id].members.clone();
                if let Some(user_id) = self.random.pick(&members) {
                    self.send(
                        "GUILD_MEMBER_REMOVE",
                        &json!({"guild_id": guild, "user": user(user_id)}),
                    );
                    self.guilds
                        .get_mut(&guild_id)
                        .unwrap()
                        .members
                        .remove(&user_id);
                }
            }
            2 => {
                let members = self.guilds[&guild_id].members.clone();
                let roles = self.guilds[&guild_id].roles.clone();
                if let (Some(user_id), Some(role_id)) =
                    (self.random.pick(&members), self.random.pick(&roles))
                {
                    self.send(
                        "GUILD_MEMBER_UPDATE",
                        &json!({"guild_id": guild, "roles": [role_id.to_string()], "user": user(user_id),
                                "nick": "n", "avatar": null, "banner": null,
                                "joined_at": "2026-01-01T00:00:00+00:00"}),
                    );
                }
            }
            3 => {
                let channel_id = self.id();
                self.send("CHANNEL_CREATE", &channel(channel_id, guild_id));
                self.guilds
                    .get_mut(&guild_id)
                    .unwrap()
                    .channels
                    .insert(channel_id);
            }
            4 => {
                // Delete only channels without threads. Discord deletes the threads of a
                // deleted channel with their own events.
                let model = &self.guilds[&guild_id];
                let parents: BTreeSet<u64> = model.threads.values().copied().collect();
                let free: BTreeSet<u64> = model.channels.difference(&parents).copied().collect();
                if let Some(channel_id) = self.random.pick(&free) {
                    self.send("CHANNEL_DELETE", &channel(channel_id, guild_id));
                    let model = self.guilds.get_mut(&guild_id).unwrap();
                    model.channels.remove(&channel_id);
                    model.messages.remove(&channel_id);
                }
            }
            5 => {
                let channels = self.guilds[&guild_id].channels.clone();
                if let Some(parent) = self.random.pick(&channels) {
                    let thread_id = self.id();
                    let mut data = thread(thread_id, guild_id, parent);
                    data["newly_created"] = json!(true);
                    self.send("THREAD_CREATE", &data);
                    self.guilds
                        .get_mut(&guild_id)
                        .unwrap()
                        .threads
                        .insert(thread_id, parent);
                }
            }
            6 => {
                let threads: BTreeSet<u64> =
                    self.guilds[&guild_id].threads.keys().copied().collect();
                if let Some(thread_id) = self.random.pick(&threads) {
                    let parent = self.guilds[&guild_id].threads[&thread_id];
                    self.send(
                        "THREAD_DELETE",
                        &json!({"id": thread_id.to_string(), "type": 11, "guild_id": guild,
                                "parent_id": parent.to_string()}),
                    );
                    let model = self.guilds.get_mut(&guild_id).unwrap();
                    model.threads.remove(&thread_id);
                    model.messages.remove(&thread_id);
                }
            }
            7 => {
                let role_id = self.id();
                self.send(
                    "GUILD_ROLE_CREATE",
                    &json!({"guild_id": guild, "role": role(role_id)}),
                );
                self.guilds
                    .get_mut(&guild_id)
                    .unwrap()
                    .roles
                    .insert(role_id);
            }
            8 => {
                let roles: BTreeSet<u64> = self.guilds[&guild_id]
                    .roles
                    .iter()
                    .copied()
                    .filter(|id| *id != guild_id)
                    .collect();
                if let Some(role_id) = self.random.pick(&roles) {
                    self.send(
                        "GUILD_ROLE_DELETE",
                        &json!({"guild_id": guild, "role_id": role_id.to_string()}),
                    );
                    self.guilds
                        .get_mut(&guild_id)
                        .unwrap()
                        .roles
                        .remove(&role_id);
                }
            }
            9 | 10 => self.message(guild_id),
            _ => {
                let members = self.guilds[&guild_id].members.clone();
                let channels = self.guilds[&guild_id].channels.clone();
                if let (Some(user_id), Some(channel_id)) =
                    (self.random.pick(&members), self.random.pick(&channels))
                {
                    let joined = self.random.below(2) == 0;
                    let channel = if joined {
                        json!(channel_id.to_string())
                    } else {
                        Value::Null
                    };
                    self.send(
                        "VOICE_STATE_UPDATE",
                        &json!({"guild_id": guild, "channel_id": channel, "user_id": user_id.to_string(),
                                "session_id": "x", "deaf": false, "mute": false, "self_deaf": false,
                                "self_mute": false, "self_video": false, "suppress": false,
                                "request_to_speak_timestamp": null}),
                    );
                }
            }
        }
    }

    fn message(&mut self, guild_id: u64) {
        let model = &self.guilds[&guild_id];
        let targets: BTreeSet<u64> = model
            .channels
            .iter()
            .chain(model.threads.keys())
            .copied()
            .collect();
        let Some(channel_id) = self.random.pick(&targets) else {
            return;
        };
        let author = self.random_user();
        let message_id = self.id();
        self.send(
            "MESSAGE_CREATE",
            &json!({"id": message_id.to_string(), "channel_id": channel_id.to_string(),
                    "guild_id": guild_id.to_string(), "author": user(author), "content": "m",
                    "timestamp": "2026-01-01T00:00:00+00:00", "edited_timestamp": null, "tts": false,
                    "mention_everyone": false, "mentions": [], "mention_roles": [], "attachments": [],
                    "embeds": [], "pinned": false, "type": 0,
                    "member": {"roles": [], "joined_at": "2026-01-01T00:00:00+00:00", "deaf": false,
                               "mute": false, "flags": 0}}),
        );

        let model = self.guilds.get_mut(&guild_id).unwrap();
        // A message with a member stores the member.
        model.members.insert(author);
        let count = model.messages.entry(channel_id).or_default();
        *count = (*count + 1).min(MESSAGE_CAPACITY);
    }

    /// Compares the cache counts with the shadow model.
    fn check(&self, phase: &str) {
        let stats = self.cache.stats();
        let channels: usize = self
            .guilds
            .values()
            .map(|guild| guild.channels.len() + guild.threads.len())
            .sum();
        let roles: usize = self.guilds.values().map(|guild| guild.roles.len()).sum();
        let members: usize = self.guilds.values().map(|guild| guild.members.len()).sum();
        let mut users: BTreeSet<u64> = self
            .guilds
            .values()
            .flat_map(|guild| guild.members.iter().copied())
            .collect();
        users.insert(CURRENT_USER);
        let messages: usize = self
            .guilds
            .values()
            .flat_map(|guild| guild.messages.values())
            .sum();

        assert_eq!(stats.guilds, self.guilds.len(), "{phase}: guilds");
        assert_eq!(stats.channels, channels, "{phase}: channels and threads");
        assert_eq!(stats.roles, roles, "{phase}: roles");
        assert_eq!(stats.members, members, "{phase}: members");
        assert_eq!(stats.users, users.len(), "{phase}: users");
        assert_eq!(stats.messages, messages, "{phase}: messages");
    }
}

fn run(guilds: usize, events: usize, seed: u64) {
    let mut soak = Soak::new(seed, (guilds as u64) * 20);

    for _ in 0..guilds {
        soak.create_guild(30, 10, 5);
    }
    soak.check("after GUILD_CREATE");

    for round in 0..events {
        soak.step();
        // Now and then a guild leaves and a new one joins.
        if round % 500 == 499 {
            let guild_id = *soak.guilds.keys().next().unwrap();
            soak.delete_guild(guild_id);
            soak.create_guild(30, 10, 5);
        }
    }
    soak.check("after the events");

    let guild_ids: Vec<u64> = soak.guilds.keys().copied().collect();
    for guild_id in guild_ids {
        soak.delete_guild(guild_id);
    }
    soak.check("after GUILD_DELETE of all guilds");
    let stats = soak.cache.stats();
    assert_eq!(
        (stats.channels, stats.members, stats.users, stats.messages),
        (0, 0, 1, 0)
    );
}

#[test]
fn soak_small() {
    run(50, 20_000, 0x5EED);
}

#[test]
#[ignore = "large: run with --ignored"]
fn soak_large() {
    run(1000, 500_000, 0xC0FFEE);
}
