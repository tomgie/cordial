# Research: Main Gateway (WebSocket)

Research date: 2026-11-05. Target API version: v10.

## Sources

- Docs site: `https://docs.discord.com/developers/...` (`discord.com/developers/docs` now redirects there).
- Source repo: `discord/discord-api-docs`. The doc folder is now `developers/`, not `docs/`.
  - `developers/events/gateway.mdx` [G]
  - `developers/events/gateway-events.mdx` [GE]
  - `developers/topics/opcodes-and-status-codes.mdx` [OP]
  - `developers/reference.mdx` [REF]
  - `developers/change-log.mdx` [CL]
  - `developers/resources/channel.mdx` [CH]

## 1. Connection lifecycle [G] [GE]

### Get Gateway / Get Gateway Bot

- `GET /gateway`: no auth. Returns `{ url }`. Cache it. Fetch again only if the cached URL fails.
- `GET /gateway/bot`: bot auth. Do not cache for long.

| Field | Type | Notes |
|---|---|---|
| url | string | WSS URL |
| shards | int | Recommended shard count |
| session_start_limit.total | int | Session starts allowed |
| session_start_limit.remaining | int | |
| session_start_limit.reset_after | int (ms) | |
| session_start_limit.max_concurrency | int | Identify requests allowed per 5 s |

### Connection URL

- Query params: `v` (API version), `encoding` (`json` or `etf`), optional `compress` (`zlib-stream` or `zstd-stream`).
- Example: `wss://gateway.discord.gg/?v=10&encoding=json`.

### Payload envelope

- Shape: `{op, d, s, t}`. `s` and `t` are null unless `op` is 0.
- Keep the latest non-null `s`.
- Each sent payload must be at most 4096 bytes. A larger payload closes with 4002.

### Hello and heartbeat

- Hello (op 10) carries `d.heartbeat_interval` in ms.
- First heartbeat after `heartbeat_interval * jitter`, with jitter in [0, 1). Then send one each `heartbeat_interval`.
- Heartbeat `d` is the last `s`, or null.
- The server answers with op 11 Heartbeat ACK.
- The server can send op 1. Reply with a heartbeat immediately.
- Zombie detection: if no ACK arrives between two heartbeats, close with a code other than 1000/1001, reconnect, and resume.

### Identify (op 2)

| Field | Type | Default | Notes |
|---|---|---|---|
| token | string | | |
| properties | `{os, browser, device}` | | `browser`/`device` = library name. `$` keys are deprecated. |
| compress? | bool | false | Payload compression (JSON only) |
| large_threshold? | int 50..250 | 50 | |
| shard? | [id, count] | | |
| presence? | Update Presence | | |
| intents | int | | Required |
| capabilities? | int | 0 | New 2026. Only flag: `CHANNEL_OBFUSCATION = 1<<15`. Temporary, testing only. |

- 1000 Identify calls per 24 h, global across shards. Resumes do not count.
- Exceeding that limit terminates all sessions and resets the bot token.
- Exceeding `max_concurrency` gives op 9.

### Ready

| Field | Notes |
|---|---|
| v | API version |
| user | Current user |
| guilds | Unavailable guilds for this shard. They arrive later as GUILD_CREATE. |
| session_id | |
| resume_gateway_url | |
| shard? | |
| application | Partial: `id`, `flags`, `flags_new` (added 2026-08) |

### Resume (op 6)

- Payload `{token, session_id, seq}`.
- Connect to `resume_gateway_url` with the same `v` and `encoding`.
- Missed events replay in order. Then `RESUMED` arrives.
- Resume on: op 7; a reconnectable close code; a disconnect with no close code; op 9 with `d=true`.

### Reconnect (op 7) and Invalid Session (op 9)

- Op 7 can arrive at any time, also before Hello.
- Op 9 `d` tells if the session can resume.
- On `d=false`, connect to the cached Get Gateway URL and identify again.
- The docs no longer give a wait time before re-identify. Use a local backoff (1..5 s random).

### Close from the client

- Codes 1000 and 1001 invalidate the session. The bot goes offline.
- Other codes, or a TCP drop, keep the session for some minutes for resume.

### Guild availability

- Guilds start unavailable. GUILD_CREATE makes them available.
- GUILD_DELETE with `unavailable: true` means an outage. Since 2026-10 it can also have other causes.

## 2. Opcodes and close codes [OP]

| Op | Name | Direction |
|---|---|---|
| 0 | Dispatch | Receive |
| 1 | Heartbeat | Both |
| 2 | Identify | Send |
| 3 | Presence Update | Send |
| 4 | Voice State Update | Send |
| 6 | Resume | Send |
| 7 | Reconnect | Receive |
| 8 | Request Guild Members | Send |
| 9 | Invalid Session | Receive |
| 10 | Hello | Receive |
| 11 | Heartbeat ACK | Receive |
| 31 | Request Soundboard Sounds | Send |
| 43 | Request Channel Info | Send (documented 2026-04) |

| Close code | Meaning | Reconnect |
|---|---|---|
| 4000 | Unknown error | yes |
| 4001 | Unknown opcode | yes |
| 4002 | Decode error (also payload > 4096 bytes, ETF atom keys) | yes |
| 4003 | Not authenticated | yes |
| 4004 | Authentication failed | no |
| 4005 | Already authenticated | yes |
| 4007 | Invalid seq; start a new session | yes (new session) |
| 4008 | Rate limited | yes |
| 4009 | Session timed out; start a new session | yes (new session) |
| 4010 | Invalid shard | no |
| 4011 | Sharding required | no |
| 4012 | Invalid API version | no |
| 4013 | Invalid intents | no |
| 4014 | Disallowed intents | no |

## 3. Encoding and compression [G]

- Payload compression: Identify `compress: true`. JSON only. Per packet zlib. Only some packets. Transport compression disables it.
- `zlib-stream`: one inflate context per connection. Buffer until the data ends with `00 00 ff ff`, then inflate.
- `zstd-stream`: one zstd context per connection. Each WS message is one gateway message but does not end a zstd frame. Call decompress until all input is consumed.
- ETF: snowflakes can be ints or strings. Sent payloads must use string keys. Atom keys cause 4002.
- JSON is the recommended default.

## 4. Rate limits [G] [CL]

- Send limit: 120 events per connection per 60 s. Exceeding it closes with 4008.
- Identify concurrency: `rate_limit_key = shard_id % max_concurrency`. Start buckets in order.
- Session starts: `total` per day, 1000 by default. Large bots get `max(2000, guild_count / 1000 * 5)`.
- Request Guild Members for all members: 1 request per guild per 30 s (enforced 2025-10-01).
- Exceeding that gives dispatch `RATE_LIMITED` `{opcode, retry_after (float s), meta: {guild_id, nonce}}`. Handle any opcode in this event.
- Presence: 5 updates per 20 s (may be a legacy note; respect it).
- Op 31 and op 43 have no documented per-opcode limit.

## 5. Sharding [G]

- `shard_id = (guild_id >> 22) % num_shards`.
- At most 2500 guilds per shard. Bots with more than that and no sharding get 4011.
- Events with no `guild_id` (DMs, entitlements, subscriptions) go only to shard 0.
- Duplicate or mixed shard counts are allowed. This permits zero-downtime rescaling.
- Large bots (over 150,000 guilds): Discord assigns a multiple. The shard count must be a multiple of it, or 4010.
- Since 2026-09-15: `GET /users/@me/guilds` requires `?shard=` for large-sharded bots.
- No maximum shard count is documented.

## 6. Intents [G]

| Bit | Intent | Events |
|---|---|---|
| 0 | GUILDS | GUILD_*, ROLE_*, CHANNEL_*, PINS, THREAD_*, STAGE_*, VOICE_CHANNEL_STATUS/START_TIME_UPDATE |
| 1 | GUILD_MEMBERS (privileged) | GUILD_MEMBER_*, THREAD_MEMBERS_UPDATE |
| 2 | GUILD_MODERATION | GUILD_AUDIT_LOG_ENTRY_CREATE, GUILD_BAN_* |
| 3 | GUILD_EXPRESSIONS | EMOJIS, STICKERS, SOUNDBOARD_* |
| 4 | GUILD_INTEGRATIONS | GUILD_INTEGRATIONS_UPDATE, INTEGRATION_* |
| 5 | GUILD_WEBHOOKS | WEBHOOKS_UPDATE |
| 6 | GUILD_INVITES | INVITE_* |
| 7 | GUILD_VOICE_STATES | VOICE_STATE_UPDATE, VOICE_CHANNEL_EFFECT_SEND |
| 8 | GUILD_PRESENCES (privileged) | PRESENCE_UPDATE |
| 9 | GUILD_MESSAGES | MESSAGE_* |
| 10 | GUILD_MESSAGE_REACTIONS | MESSAGE_REACTION_* |
| 11 | GUILD_MESSAGE_TYPING | TYPING_START |
| 12 | DIRECT_MESSAGES | MESSAGE_*, CHANNEL_PINS_UPDATE |
| 13 | DIRECT_MESSAGE_REACTIONS | MESSAGE_REACTION_* |
| 14 | DIRECT_MESSAGE_TYPING | TYPING_START |
| 15 | MESSAGE_CONTENT (privileged) | No events. Fills content fields. |
| 16 | GUILD_SCHEDULED_EVENTS | GUILD_SCHEDULED_EVENT_* |
| 20 | AUTO_MODERATION_CONFIGURATION | AUTO_MODERATION_RULE_* |
| 21 | AUTO_MODERATION_EXECUTION | AUTO_MODERATION_ACTION_EXECUTION |
| 24 | GUILD_MESSAGE_POLLS | MESSAGE_POLL_VOTE_* |
| 25 | DIRECT_MESSAGE_POLLS | MESSAGE_POLL_VOTE_* |

- Without MESSAGE_CONTENT: `content`, `embeds`, `attachments`, `components` are empty and `poll` is absent. Exceptions: own messages, DMs, mentions, context menu targets.
- Privileged intent review: required at 10,000 users or more, renewed each year (2026-06-10).
- Invalid intents close with 4013. Unapproved privileged intents close with 4014.

## 7. Dispatch events [GE]

| Area | Events | Requirement |
|---|---|---|
| Lifecycle | READY, RESUMED, RATE_LIMITED (2025) | none |
| Commands | APPLICATION_COMMAND_PERMISSIONS_UPDATE | none |
| Interactions | INTERACTION_CREATE | none |
| AutoMod | AUTO_MODERATION_RULE_CREATE/UPDATE/DELETE, AUTO_MODERATION_ACTION_EXECUTION | intents + MANAGE_GUILD |
| Channels | CHANNEL_CREATE/UPDATE/DELETE, CHANNEL_PINS_UPDATE | GUILDS |
| Channels | CHANNEL_INFO (reply to op 43) | none |
| Channels | VOICE_CHANNEL_STATUS_UPDATE, VOICE_CHANNEL_START_TIME_UPDATE (2026-04) | GUILDS |
| Threads | THREAD_CREATE/UPDATE/DELETE, THREAD_LIST_SYNC, THREAD_MEMBER_UPDATE, THREAD_MEMBERS_UPDATE | GUILDS (+ GUILD_MEMBERS) |
| Monetization | ENTITLEMENT_CREATE/UPDATE/DELETE, SUBSCRIPTION_CREATE/UPDATE/DELETE | none; shard 0 |
| Guild | GUILD_CREATE/UPDATE/DELETE, GUILD_ROLE_CREATE/UPDATE/DELETE | GUILDS |
| Guild | GUILD_AUDIT_LOG_ENTRY_CREATE | GUILD_MODERATION + VIEW_AUDIT_LOG |
| Guild | GUILD_BAN_ADD/REMOVE | GUILD_MODERATION |
| Guild | GUILD_EMOJIS_UPDATE, GUILD_STICKERS_UPDATE, GUILD_SOUNDBOARD_SOUND_CREATE/UPDATE/DELETE, GUILD_SOUNDBOARD_SOUNDS_UPDATE | GUILD_EXPRESSIONS |
| Guild | SOUNDBOARD_SOUNDS (reply to op 31) | none |
| Guild | GUILD_INTEGRATIONS_UPDATE, INTEGRATION_CREATE/UPDATE/DELETE | GUILD_INTEGRATIONS |
| Guild | GUILD_MEMBER_ADD/UPDATE/REMOVE | GUILD_MEMBERS |
| Guild | GUILD_MEMBERS_CHUNK (reply to op 8) | none |
| Guild | GUILD_SCHEDULED_EVENT_CREATE/UPDATE/DELETE/USER_ADD/USER_REMOVE | GUILD_SCHEDULED_EVENTS |
| Invites | INVITE_CREATE/DELETE | GUILD_INVITES |
| Messages | MESSAGE_CREATE/UPDATE/DELETE, MESSAGE_DELETE_BULK | GUILD_MESSAGES / DIRECT_MESSAGES |
| Messages | MESSAGE_REACTION_ADD/REMOVE/REMOVE_ALL/REMOVE_EMOJI | reaction intents |
| Polls | MESSAGE_POLL_VOTE_ADD/REMOVE | poll intents |
| Presence | PRESENCE_UPDATE | GUILD_PRESENCES |
| Typing | TYPING_START | typing intents |
| User | USER_UPDATE | none |
| Voice | VOICE_STATE_UPDATE, VOICE_CHANNEL_EFFECT_SEND | GUILD_VOICE_STATES |
| Voice | VOICE_SERVER_UPDATE (null endpoint = reallocation) | none |
| Webhooks | WEBHOOKS_UPDATE | GUILD_WEBHOOKS |
| Stage | STAGE_INSTANCE_CREATE/UPDATE/DELETE | GUILDS |

Payload notes:

- MESSAGE_CREATE/UPDATE add `guild_id?`, `member?`, `mentions` with partial member, and `channel_type?` (2026-06).
- PRESENCE_UPDATE `user` can be partial. Field types are not validated. Parse it leniently.
- GUILD_CREATE adds `joined_at`, `large`, `member_count`, `voice_states`, `members`, `channels`, `threads`, `presences`, `stage_instances`, `guild_scheduled_events`, `soundboard_sounds`.
- The docs say undocumented fields can change at any time.

## 8. Send events [GE]

- Update Presence (op 3): `since`, `activities[]`, `status` (online, dnd, idle, invisible, offline), `afk`.
  - Activity types: 0 Playing, 1 Streaming, 2 Listening, 3 Watching, 4 Custom, 5 Competing.
  - Bots can set only `name`, `state`, `type`, `url`.
  - Received activities also have `status_display_type`, `details_url`, `state_url`. `client_status` has `vr` (2026-08).
- Update Voice State (op 4): `{guild_id, channel_id (null = leave), self_mute, self_deaf}`.
- Request Guild Members (op 8): `guild_id`, `query` or `user_ids`, `limit`, `presences?`, `nonce?` (max 32 bytes).
  - Reply: GUILD_MEMBERS_CHUNK with up to 1000 members, `chunk_index`, `chunk_count`, `not_found?`, `presences?`, `nonce?`.
- Request Soundboard Sounds (op 31): `{guild_ids}`. Reply: one SOUNDBOARD_SOUNDS per guild.
- Request Channel Info (op 43): `{guild_id, fields: ["status", "voice_start_time"]}`. Reply: CHANNEL_INFO.

## 9. Changes since 2024 that matter

- API versions: v10 and v9 available. v8..v6 deprecated (v6 is still the default). v5..v3 discontinued. Always send `v=10`.
- Channel obfuscation (breaking, mandatory 2026-11-16):
  - Hidden channels arrive with name `___hidden___`, flag `CHANNEL_OBFUSCATED = 1<<17`, and reduced fields.
  - Only `id`, `type`, `position`, `parent_id` are reliable.
  - Access gain sends CHANNEL_UPDATE with full data.
  - `GET /guilds/{id}/channels` omits these channels.
  - Test opt-in through Identify `capabilities: 1<<15`. The opt-in mechanism will change before GA.
- RATE_LIMITED dispatch and the op 8 limit (2025).
- Custom status (type 4) can be absent from PRESENCE_UPDATE because of privacy settings (2026-09-17).
- Voice gateway versions below 4 discontinued (2024-11). Voice close 4017 means DAVE is required.

## Open points

- Re-identify delay after op 9.
- Per-opcode limits for op 31 and op 43.
- Maximum shard count.
- Date of `zstd-stream` addition.
- Whether "5 presence updates per 20 s" is still valid.
