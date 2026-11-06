# Cache

The cache keeps Discord state in memory: guilds, channels, roles, members, and more. It
takes gateway events as input. The bot updates the cache before it runs the handlers of
an event, so a handler sees the new state.

The cache is off by default. Turn it on with `BotBuilder::cache` and select the resource
types. A resource type that you do not select costs no memory.

## Resource types

| `ResourceTypes` | Keeps | Intent | Privileged |
|---|---|---|---|
| `GUILD` | Guilds (without their lists) | `GUILDS` | no |
| `CHANNEL` | Channels and threads | `GUILDS` | no |
| `ROLE` | Roles | `GUILDS` | no |
| `MEMBER` | Members (without their user) | `GUILD_MEMBERS` | yes |
| `USER` | Users of members, message authors, the current user | `GUILDS` | no |
| `EMOJI` | Emojis | `GUILD_EXPRESSIONS` | no |
| `STICKER` | Stickers | `GUILD_EXPRESSIONS` | no |
| `VOICE_STATE` | Voice states | `GUILD_VOICE_STATES` | no |
| `PRESENCE` | Presences | `GUILD_PRESENCES` | yes |
| `MESSAGE` | Recent messages of each channel | `GUILD_MESSAGES`, `DIRECT_MESSAGES`, and the reaction intents | no |
| `STAGE_INSTANCE` | Stage instances | `GUILDS` | no |
| `SCHEDULED_EVENT` | Scheduled events | `GUILD_SCHEDULED_EVENTS` | no |
| `SOUNDBOARD_SOUND` | Soundboard sounds | `GUILD_EXPRESSIONS` | no |

The bot adds the intents of the cache to the computed intents. Without the privileged
`GUILD_MEMBERS` intent, the member events do not arrive. Then the member cache is not
complete.

```rust
{{#include ../../../crates/cordial/tests/guide/cache.rs:setup}}
```

## Reads

Each read gives an `Arc` value or a list of `Arc` values. A read never holds a lock, so a
value is safe to keep across `.await`.

| Method | Gives |
|---|---|
| `guild(guild_id)` | A `CachedGuild`. Its lists of channels, roles, and members are empty. |
| `channel(channel_id)`, `guild_channels(guild_id)` | Channels and threads. |
| `role(role_id)`, `guild_roles(guild_id)` | `CachedRole` values. |
| `member(guild_id, user_id)`, `guild_members(guild_id)` | `CachedMember` values. Read the user with `user`. |
| `user(user_id)`, `current_user()` | Users. |
| `voice_state(guild_id, user_id)`, `voice_channel_states(channel_id)` | Voice states. |
| `message(channel_id, message_id)`, `channel_messages(channel_id)` | Cached messages. |
| `stats()` | The number of cached objects of each kind, for metrics. |

The cache keeps each object one time. A member does not hold its user: the cache stores
the user once and all guilds share it.

`Cache::update` gives the previous value of a changed or removed object, as `Previous`.
Use the cache without the framework to get it.

## Permissions from the cache

The cache computes permissions with the cached guild, roles, member, and channel.

| Method | Gives |
|---|---|
| `guild_permissions(guild_id, user_id)` | The guild-level permissions of a member. |
| `channel_permissions(channel_id, user_id)` | The permissions in a channel or a thread. |

The calculation follows the rules of Discord: the owner has all permissions, the
`ADMINISTRATOR` permission gives all permissions, the channel overwrites apply in order,
and a timed-out member keeps only `VIEW_CHANNEL` and `READ_MESSAGE_HISTORY`. For a thread,
the overwrites of the parent channel apply.

The calculation needs the resource types `GUILD`, `ROLE`, `CHANNEL`, and `MEMBER`.
A `PermissionError` tells which object is not in the cache.

```rust
{{#include ../../../crates/cordial/tests/guide/cache.rs:permissions}}
```

In an interaction, `ctx.member_permissions()` gives the permissions of the user without
the cache. Discord computes them.

## Obfuscated channels

Discord obfuscates the channels that the bot cannot view. An obfuscated channel arrives
over the gateway with the flag `CHANNEL_OBFUSCATED`. Only its ID, type, position, and
parent ID are correct.

- `Channel::is_obfuscated` tells if a channel is obfuscated.
- `channel_permissions` gives `PermissionError::Obfuscated` for such a channel, because its
  overwrites are not known.

From 2026-11-16, the obfuscation is mandatory. `GET /guilds/{id}/channels` then omits the
hidden channels.

## The message ring buffer

The `MESSAGE` resource keeps the recent messages of each channel in a ring buffer. When
the buffer is full, a new message removes the oldest message.

- The default size is 100 messages for each channel.
- `BotBuilder::cache_messages(capacity)` sets the size. A size of 0 keeps no messages.
- `channel_messages` gives the messages from the oldest to the newest.
- Message updates, deletes, and reactions change the cached messages.

`cache_messages` alone turns on the cache with only the `MESSAGE` resource. After
`cache(...)`, it changes only the size. Put `ResourceTypes::MESSAGE` in `cache(...)` to
keep messages and other resources.
