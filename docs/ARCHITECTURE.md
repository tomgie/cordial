# Architecture

Status: draft 1, 2026-11-05. Target: Discord API v10, voice gateway v8, DAVE protocol v1.

This document gives the structure of the library and the reasons for each decision.
The facts about Discord come from the research notes in [research/](research/).
The work order is in [ROADMAP.md](ROADMAP.md).

## 1. Goals

1. Cover the full bot surface of the Discord API: HTTP, gateway, interactions, voice, and OAuth2.
2. Let a user take only the layer they need. An HTTP-only bot must not compile a gateway.
3. Survive API changes. An unknown field, enum value, flag bit, or event must not break a bot.
4. Make the correct use the easy use. Examples: rate limits, intents, limits on message size, and permission checks.
5. Make each protocol testable without a network connection.
6. Give a high-level framework (commands, components, modals) that stays optional.

## 2. Non-goals

- User accounts ("self-bots"). They violate the Discord terms.
- Video and Go Live. Discord does not permit them for bots.
- The Embedded App SDK (Activities client side). It runs in a browser, not in a bot.
- The Discord Social SDK game client. The library covers only its HTTP endpoints (lobbies, identity profiles).
- Async runtimes other than Tokio for the drivers. The protocol cores stay runtime-free, so others can write drivers.

## 3. Crate layout

The project is a Cargo workspace. Each crate is one layer. The crate name prefix is `cordial`.

```text
                           +----------------------------+
                           |  cordial  (facade)         |
                           |  Bot, re-exports, features |
                           +-------------+--------------+
                                         |
          +------------------+-----------+-----------+------------------+
          |                  |                       |                  |
+---------v--------+ +-------v--------+   +----------v-------+ +--------v---------+
| framework        | | cache          |   | voice            | | endpoint         |
| router, commands | | in-memory      |   | voice gateway,   | | signed HTTP      |
| components,      | | state, update  |   | UDP, crypto,     | | requests from    |
| collectors       | | from events    |   | DAVE, audio      | | Discord          |
+---+----------+---+ +-------+--------+   +---+----------+---+ +--------+---------+
    |          |             |                |          |              |
    | +--------v-------------v----------------v--+       |              |
    | | gateway                                  |       |              |
    | | shard core (sans-IO), driver, manager    |       |              |
    | +------------------+-----------------------+       |              |
    |                    |                               |              |
+---v--------------------v-------------------------------v--------------v---+
| http                                                                      |
| endpoints, rate limiter, transport, OAuth2                                |
+------------------------------------+--------------------------------------+
                                     |
+------------------------------------v--------------------------------------+
| model                                                                     |
| IDs, objects, enums, flags, events, permissions, builders, validation     |
+---------------------------------------------------------------------------+

macros (proc-macro crate) -> used by framework
```

| Crate | Depends on | Purpose |
|---|---|---|
| `cordial-model` | serde | All data types. No I/O. Compiles to `wasm32`. |
| `cordial-http` | model | HTTP client, all endpoints, rate limits, OAuth2. |
| `cordial-gateway` | model, http | Gateway shards and the shard manager. |
| `cordial-cache` | model | In-memory state. Updates from gateway events. |
| `cordial-endpoint` | model | Receives interactions and webhook events over HTTP. Verifies Ed25519 signatures. |
| `cordial-voice` | model, gateway | Voice connections: send and receive audio. |
| `cordial-macros` | (proc-macro) | Derive macros for commands, options, choices, and modals. |
| `cordial-framework` | all above except voice | Event router, command system, component router, collectors. |
| `cordial` | all, by feature | Facade. One dependency for most users. |
| `xtask` | (dev only) | Spec drift check, code generation helpers, release tasks. |

Reasons:

- The `model` crate has no I/O. Thus the endpoint crate can run in a serverless function, and tests do not need a network.
- The `cache` crate does not depend on `gateway`. It takes events as input. Thus a user can feed it from any source.
- The `endpoint` crate does not depend on `http`. A serverless bot that only replies inline needs no HTTP client.
- `voice` is separate because it brings C libraries (Opus) and much cryptography.

## 4. Cross-cutting decisions

### 4.1 Async runtime

- Use Tokio for all drivers (gateway, voice, framework).
- Keep the gateway and voice protocol logic in sans-IO cores (section 7.1). The cores take bytes and time as input and give actions as output.
- Reason: Tokio is the standard in the ecosystem. One runtime keeps the code simple. The sans-IO cores keep the protocol logic portable and testable.

### 4.2 Typed IDs

```rust
pub struct Id<T> { value: NonZeroU64, _marker: PhantomData<fn() -> T> }

pub type UserId = Id<UserMarker>;
pub type GuildId = Id<GuildMarker>;
pub type ChannelId = Id<ChannelMarker>;
```

- An ID of one kind cannot go where an ID of another kind is necessary.
- Serialize as a JSON string. Deserialize from a string or an integer (Discord sends integers in some places).
- `Id::timestamp()` gives the creation time. `Id::from_timestamp()` makes a pagination cursor.
- `NonZeroU64` makes `Option<Id<T>>` the same size as `Id<T>`.

### 4.3 Forward compatibility

Discord adds fields, enum values, flag bits, and events without a new API version. The library must accept them.

| Item | Rule |
|---|---|
| Struct | `#[non_exhaustive]`. Never use `deny_unknown_fields`. |
| Integer enum | Has an `Unknown(u16)` variant. The raw value goes back out unchanged. |
| String enum | Has an `Unknown(String)` variant (guild features, embed types). |
| Bit flags | Keep unknown bits. Serialize them back unchanged. |
| Gateway event | Unknown names become `Event::Unknown { name, data: Box<RawValue> }`. |
| Component | Unknown component types become `Component::Unknown { kind, raw }`. |
| Error code | `JsonErrorCode` has `Unknown(u32)`. |

- A macro (`discord_enum!`) generates the enum, the `Unknown` variant, serde, `Display`, and `From<raw>`. This keeps the rule the same in all places.
- Bit flags use a 64-bit store (`u64`). Permissions already use bit 52.

### 4.4 Optional, nullable, and patch fields

The docs use three states: absent (`field?`), null (`?T`), and value.

| Use | Rust type |
|---|---|
| Response, optional only | `Option<T>` with `#[serde(default)]` |
| Response, nullable only | `Option<T>` |
| Response, optional and nullable, where the difference matters | `Nullable<T>` = `Absent`, `Null`, `Value(T)` |
| Request PATCH field | `Patch<T>` = `Keep` (not sent), `Clear` (null), `Set(T)` |

- Special case: role tags use "present and null means true". Use a custom deserializer that gives `bool`.

### 4.5 Partial objects

Discord sends reduced forms of objects in many places.

- Make a separate type when the docs define the field set: `UnavailableGuild`, `PartialMember` (interactions), `PartialChannel` (interactions), `PartialRole` (invites), `MessageSnapshot`, `StickerItem`.
- Use one struct with optional fields when the shape changes by context but the docs give no fixed set: `Channel`, `Message`.
- `Channel` keeps a `kind: ChannelType` field and optional fields for each kind. Typed views (`channel.as_text()`, `channel.as_thread()`) give checked access. Reason: obfuscated channels (mandatory from 2026-11-16) remove most fields, so a strict enum per kind fails.

### 4.6 Errors

- Each crate has its own error type with a `kind()` method. Callers can match on the kind without parsing text.
- HTTP API errors keep: status, `JsonErrorCode`, message, and the nested field-error tree.
- Validation errors (section 5.4) are separate from API errors.
- Gateway errors separate fatal close codes (4004, 4010..4014) from codes that permit a reconnect.
- Use `thiserror` in library crates. Do not use `anyhow` in library crates.

### 4.7 Secrets

- A `Token` type holds the bot token. `Debug` and `Display` print `Token(<redacted>)`.
- The token memory is zeroed on drop (`zeroize`).
- Logs never contain a token or an interaction token.

### 4.8 Observability

- Use `tracing` spans and events in all crates. Spans carry the shard ID, the route, and the rate-limit bucket.
- An optional `metrics` feature gives counters: requests, 429 responses, invalid-request budget, gateway latency, reconnects, events per second.

### 4.9 Serialization

- Default: `serde_json`.
- Optional feature `simd-json` for faster gateway decode.
- ETF (Erlang term format) is not in version 1. Reason: JSON is the documented default. ETF gives a small gain for a large cost.

## 5. Model crate

### 5.1 Contents

| Module | Contents |
|---|---|
| `id` | `Id<T>`, markers, snowflake time helpers |
| `time` | `Timestamp` (ISO 8601), durations |
| `user`, `guild`, `channel`, `message`, ... | One module for each Discord resource |
| `permissions` | `Permissions` flags and the calculator (section 5.3) |
| `gateway` | Opcodes, payloads, `Intents`, `Event` enum, one struct for each dispatch event |
| `interaction` | Interaction, command data, callback types and response bodies |
| `component` | Components V1 and V2, modal components |
| `cdn` | Image URL builder for each CDN path, with format and size |
| `builder` | Builders for messages, embeds, components, modals, commands |
| `validate` | Limit checks (section 5.4) |
| `voice` | Voice opcodes and payloads (shared by gateway and voice crates) |

### 5.2 Event types

- One struct for each dispatch event (`MessageCreate`, `GuildMemberUpdate`, ...).
- One `Event` enum that wraps them, plus `Unknown`.
- `EventType` is a flag set with one bit for each event. The router and the cache use it to say which events they need.
- A table maps each event to the intent that enables it. The framework uses it to compute the intents (section 9.2).

### 5.3 Permission calculator

- A pure function: `compute_permissions(guild_owner, everyone_role, member_roles, overwrites, member_id) -> Permissions`.
- It follows the algorithm in [research/02-http.md](research/02-http.md) section 5, with the implicit rules (no VIEW_CHANNEL, no SEND_MESSAGES, timeout, threads).
- An obfuscated channel gives "no VIEW_CHANNEL".
- Property tests compare it with the server-side permission endpoints added on 2026-10-30.

### 5.4 Client-side validation

Builders check documented limits before a request goes out.

- Examples: content 2000 characters, 10 embeds, 6000 embed characters in total, 40 components, `custom_id` 1..100, command name regex, 25 choices.
- Reason: Discord counts invalid requests (401, 403, 429) against a budget of 10,000 in 10 minutes. Other bad requests waste time and quota.
- Validation is on by default. A user can turn it off for one request or for the client, when Discord changes a limit before the library does.

## 6. HTTP crate

### 6.1 Endpoint definitions

Each of the 245 documented operations is one request type.

```rust
pub trait Endpoint {
    type Response: DeserializeOwned;

    fn route(&self) -> Route;
    fn body(&self) -> Result<RequestBody, ValidationError>;
    fn audit_reason(&self) -> Option<&str> { None }
}

pub struct Route {
    pub method: Method,
    pub template: &'static str,  // "/channels/{channel_id}/messages"
    pub path: String,            // "/channels/1234/messages"
    pub major: MajorParam,       // Channel(id), Guild(id), Webhook(id, token_hash), None
    pub global_limit: bool,      // false for interaction callback and follow-up routes
}
```

- Request types have builder methods and implement `IntoFuture`. The user writes one expression:

```rust
let message = http
    .create_message(channel_id)
    .content("Hello")
    .reply_to(message_id)
    .await?;
```

- One type for each endpoint makes coverage measurable. The `xtask` compares the route templates with the Discord OpenAPI specification (section 12.1).
- `RequestBody` is `None`, `Json(bytes)`, `Form(bytes)` (OAuth2), or `Multipart { payload_json, files }`.

### 6.2 Transport

- An internal `Transport` trait sends a request and gives a response. The default uses `reqwest` with `rustls`.
- Reason for the trait: tests use a fake transport. A `wasm32` transport (fetch) can come later.
- Settings: base URL (for an HTTP proxy service), timeout, `User-Agent` (`DiscordBot (<url>, <version>)`), and the TLS backend (`rustls` default, `native-tls` feature).

### 6.3 Rate limiter

The design follows [research/02-http.md](research/02-http.md) section 2.

```text
request --> route key (method + template + major param)
              |
              v
        bucket hash known? --no--> temporary bucket, one request at a time
              | yes
              v
        bucket state [hash + major param]: remaining, reset time, wait queue
              |
              v
        global limiter (50/s, skipped for interaction routes)
              |
              v
          transport --> update bucket from headers
                    --> 429: scope "global" pauses all, "user" pauses the bucket,
                             "shared" waits and does not count as a client fault
```

- A `RateLimiter` trait makes the limiter replaceable. Implementations:
  - `InMemory` (default).
  - `None`, for use behind an external HTTP proxy that does the rate limiting for many processes.
- Each bucket has a FIFO queue. Thus requests on one route keep their order.
- An invalid-request counter tracks 401, 403, and 429 responses in a moving 10-minute window. Over a set threshold, the client slows down and logs a warning. A 401 marks the token as not valid and stops all requests.
- Emoji routes get a cautious mode, because their headers are not reliable.
- Some limits arrive as JSON error codes (20016, 20028, 30046, 110000). The client gives them as typed errors with the `retry_after` value.

### 6.4 Retries

- Retry 502, 503, 504, and connection errors with backoff for idempotent methods (GET, PUT, DELETE, PATCH).
- Retry POST only when it is safe: Create Message with `enforce_nonce`, and the interaction callback never.
- 429 responses always wait and retry, up to a set limit. The user can choose to get the error instead.

### 6.5 OAuth2

- An `OAuth2Client` for the authorization code grant (with optional PKCE), refresh, revoke, and client credentials.
- It checks the `iss` value in the redirect (added 2026-10-14).
- URL builders for bot authorization and advanced bot authorization (`permissions`, `guild_id`, `integration_type`).
- A `BearerClient` uses the same endpoint types with a Bearer token. Some endpoints need it, for example the command permission PUT.

## 7. Gateway crate

### 7.1 Sans-IO shard core

```rust
pub struct ShardCore { /* session, sequence, heartbeat state, send limiter */ }

impl ShardCore {
    pub fn connected(&mut self, now: Instant);
    pub fn received(&mut self, frame: &[u8], now: Instant) -> Result<(), ReceiveError>;
    pub fn closed(&mut self, code: Option<u16>, now: Instant);
    pub fn send(&mut self, command: Command) -> Result<(), SendError>;
    pub fn poll_action(&mut self, now: Instant) -> Option<Action>;
    pub fn next_deadline(&self) -> Option<Instant>;
}

pub enum Action {
    Connect { url: Url },
    SendFrame(Vec<u8>),
    Close { code: u16 },
    Dispatch(RawDispatch),
    Fatal(FatalError),
}
```

- The core does the protocol: Hello, Identify, Resume, heartbeat with jitter, zombie detection, op 7, op 9, and close codes.
- The core has no socket and no clock. Tests drive it with scripted frames and fake time.
- The driver (`Shard`) owns the WebSocket (`tokio-tungstenite`), the timers, and decompression. It is small.

### 7.2 Connection rules

| Situation | Action |
|---|---|
| Op 7 Reconnect | Close with a code other than 1000/1001, resume at `resume_gateway_url` |
| Op 9, `d = true` | Resume |
| Op 9, `d = false` | Wait 1..5 s at random, identify again at the cached gateway URL |
| Close 4007, 4009 | New session (identify) |
| Close 4004, 4010..4014 | Stop the shard. Give a fatal error. |
| Other close or TCP drop | Resume |
| No heartbeat ACK | Close with a code other than 1000/1001, resume |

### 7.3 Decompression and decode

- Transport compression: `zlib-stream` (default, pure Rust through `flate2` with the `zlib-rs` backend) or `zstd-stream` (feature `zstd`).
- One decompression context for each connection. Reset it on reconnect.
- Decode in two steps:
  1. Parse the envelope `{op, s, t}` and keep `d` as `RawValue`.
  2. Parse `d` into a typed event only when a consumer wants that event type.
- Reason: a large bot receives many events (for example PRESENCE_UPDATE and TYPING_START) that no handler uses. Skipping them saves much CPU.

### 7.4 Send rate limit

- Each shard has a limiter of 120 commands for each 60 seconds.
- The limiter keeps 3 slots for heartbeats. Thus a busy bot cannot block its own heartbeat and lose the session.
- The limiter also handles the `RATE_LIMITED` dispatch event for op 8 (1 all-members request per guild in 30 s). Requests return a typed error with `retry_after`.
- Each command is checked against the 4096-byte payload limit before it is sent.

### 7.5 Shard manager

- `ShardManager` starts and supervises N shards. It reads `GET /gateway/bot` for the recommended count and `max_concurrency`.
- An `IdentifyQueue` trait controls when a shard can identify:
  - `LocalQueue` (default): buckets by `shard_id % max_concurrency`, one identify for each bucket each 5 s, in order.
  - A user can give a shared queue (for example with Redis) when shards run in many processes.
- The manager tracks the daily session start limit. It refuses to start shards when the remaining count is low, because exceeding the limit resets the bot token.
- Session export and import: the manager can write `(session_id, sequence, resume_url)` for each shard on shutdown. A new process resumes these sessions and receives the missed events.
- Resharding (later phase): start a second set of shards with the new count, wait for READY on all, then close the old set. Discord permits duplicate sessions.

### 7.6 Event stream

- Each shard gives a `Stream` of `Result<Event, ReceiveError>` through a bounded channel. The bound gives back-pressure.
- The manager merges all shards into one stream. Each item carries the shard ID.
- Events that have no guild (DMs, entitlements, subscriptions) come only on shard 0. The docs say so. The stream does not hide it.

## 8. Cache crate

- The cache is optional. It takes events as input: `cache.update(&event) -> CacheUpdate`.
- The user selects resource types (`ResourceType::GUILD | CHANNEL | MEMBER | ...`). The cache asks only for the events it needs.
- Concurrent maps store `Arc<CachedT>`. A read gives an `Arc` clone.
  - Reason: a read never holds a lock across an `.await`. This prevents a common deadlock.
- Cached types are compact. For example, `CachedMember` stores a user ID, not a full user. Users are stored once and shared.
- An update returns the previous value. The framework can give "old and new" to handlers of update events.
- Obfuscated channels: store them with the flag. The permission helper treats them as not visible. A later CHANNEL_UPDATE with full data replaces them.
- Messages: an optional ring buffer for each channel with a set size (default off).
- Permission helpers: `cache.permissions_in(channel_id, user_id)`. They use the calculator in section 5.3.
- Version 1 has only the in-memory backend. A trait for other backends (for example Redis) can come later, when a real need shows its shape.

## 9. Framework crate

### 9.1 Bot and state

```rust
#[tokio::main]
async fn main() -> Result<(), Error> {
    let bot = Bot::builder(Token::from_env("DISCORD_TOKEN")?)
        .state(AppState::default())
        .command::<Ban>(ban)
        .component("vote:{poll_id}:{choice}", on_vote)
        .on::<MessageCreate>(on_message)
        .build()?;

    bot.run().await
}
```

- User state is a generic type parameter `S`, like `axum` state. There is no type map. Reason: the compiler checks state access.
- `Context<S>` gives: the HTTP client, the cache (if on), the shard handle, the state, and the voice manager (if on).

### 9.2 Event router

- `on::<E>(handler)` registers a typed handler for one event type.
- The router computes the required intents and the `EventType` filter from its handlers. The builder warns when a handler needs a privileged intent that is not on.
- Each event runs in a new task by default. `EventOrder::PerShard` runs the events of each shard in order. All events of a guild come from one shard, so this keeps the order of each guild. Interactions always run at the same time, because of the 3-second limit.
- A handler error goes to one error hook. A panic in a handler does not stop the bot.

Intent check (on by default, configurable to "warn", "error", or "off"):

| Check | Source | Result |
|---|---|---|
| A handler or the cache needs an intent that is not set | Router handlers, cache resource types, event-to-intent table | Warning, or error in strict mode |
| An intent is set, but nothing uses it | The same table | Info: remove it to get fewer events |
| A requested privileged intent is not on in the Developer Portal | `GET /applications/@me` flags (`GATEWAY_*` and `GATEWAY_*_LIMITED`) | Error before connect, with the name of the intent and the portal page |
| Message handlers without `MESSAGE_CONTENT` | Router handlers | One warning: content fields will be empty except for mentions, DMs, and own messages |
| Close code 4013 or 4014 | Gateway | Fatal error that names the intents, from the application flags |
| Unknown dispatch event | Gateway | One debug log for each event name |

- The check cannot predict privileged intent approval. Approval needs a review at 10,000 users, renewed each year.
- The gateway crate gives the close-code part. The framework gives the parts that need handler knowledge.

### 9.3 Commands

```rust
#[derive(Command)]
#[command(name = "ban", description = "Ban a member", default_member_permissions = "BAN_MEMBERS")]
struct Ban {
    #[option(description = "The member to ban")]
    user: ResolvedUser,
    #[option(description = "The reason", max_length = 512)]
    reason: Option<String>,
}

async fn ban(ctx: CommandContext<AppState>, args: Ban) -> Result<(), Error> {
    ctx.http()
        .ban_member(ctx.guild_id()?, args.user.id)
        .reason(args.reason.as_deref())
        .await?;

    ctx.reply("The member is banned.").ephemeral().await
}
```

- Derive macros:
  - `Command` on a struct: the options and the definition.
  - `CommandGroup` on an enum: subcommands and groups.
  - `Choice` on an enum: fixed choices.
  - `Modal` on a struct: a modal with Label components, and parsing of the submit data.
- The macros generate the command definition and the parser. The same source of truth gives both. Thus they cannot disagree.
- Autocomplete: the option has `#[option(autocomplete)]`. The handler is registered with `BotBuilder::autocomplete::<C>(handler)` and gives the choices. Reason: the macro then does not need the state type of the bot.
- Localization: a `Localizer` trait gives names and descriptions for each locale.
- Checks: permission checks, guild-only checks, cooldowns, and user checks run before the handler.
- Context menu commands (USER, MESSAGE) and the Entry Point command are supported.

Command registration (for global commands and for each guild scope):

1. Build the local definitions from all registered commands.
2. Get the current commands from Discord (`?with_localizations=true`).
3. Normalize both sides (see below).
4. Match local and remote commands by the key `(type, name)`. Discord makes this key unique in each scope.
5. Make a plan. Each command gets one action:

| Action | Condition | Request |
|---|---|---|
| Unchanged | Same key, same normalized content | None |
| Update | Same key, different content | `PATCH .../commands/{id}` |
| Rename | One of the local `previous_names` matches a remote command | `PATCH .../commands/{id}` (keeps the ID and the permissions) |
| Create | Local command with no remote match | `POST .../commands` |
| Delete | Remote command with no local match | `DELETE .../commands/{id}` |

6. Apply the plan. The default strategy is `Granular` (one request for each action).

Normalization:

- Fill the documented defaults on both sides: `type` 1, `nsfw` false, option `required` false, `integration_types`, `contexts`, `default_member_permissions`.
- Treat absent, null, and empty lists and maps as equal.
- Remove fields that only Discord sets: `id`, `application_id`, `guild_id`, `version`.
- Compare deprecated fields (`dm_permission`, `default_permission`) only when the local definition sets them.

Rules:

- Never delete the Entry Point command (type 4) unless the user asks for it.
- A dry-run mode gives the plan and sends nothing.
- The plan is in the log: "created 1, updated 2, renamed 0, deleted 1, unchanged 14".
- An update or a rename is a PATCH. A PATCH keeps each field that it does not send, so the edit always sends `nsfw`, the options, `default_member_permissions` (null for everyone), and empty localization maps to clear old values.
- `integration_types`, `contexts`, and `handler` are compared only when the local definition sets them. The command default of `integration_types` is the app configuration, which one command cannot see.
- If one request fails, stop and report the actions that were applied. The next run makes a new plan from the new remote state, so the process always converges.
- Strategy `Bulk` sends one `PUT` (bulk overwrite) when the plan has changes. Use it when a scope has many changes, for example the first registration of a large bot.
- Strategy `Auto` picks `Bulk` when the number of actions is more than a set threshold, else `Granular`.

Reasons for `Granular` as the default:

- Unchanged commands stay untouched. Discord does not change their version.
- A rename can keep the command ID and its permission overwrites. A delete and a create lose them.
- The log shows exactly what changed.

Note: only new commands count against the limit of 200 command creates per day, with both strategies.

### 9.4 Interaction responses

- The context tracks the response state: not acknowledged, deferred, replied.
- `reply()` picks the correct call for the state: callback, edit original, or follow-up.
- A timer warns (in the log) when a handler has not acknowledged after 2.5 s. An option defers automatically.
- Components V2 messages use a builder that checks the 40-component limit and the flag rules.

### 9.5 Component router and collectors

- Component handlers match `custom_id` patterns, for example `"vote:{poll_id}:{choice}"`. The pattern gives typed parameters.
- Collectors wait for events that match a filter, with a timeout. Example: wait for the next button press on one message.
- Reason: a `custom_id` carries state across restarts. Collectors serve short, live conversations.

## 10. Endpoint crate (HTTP interactions and webhook events)

- `verify(public_key, signature, timestamp, body) -> Result<(), VerifyError>`. Uses `ed25519-dalek` (constant time).
- Reject a request when the timestamp is too old (configurable). This limits replays.
- Interactions: PING gives PONG. Other types go to a handler that returns an `InteractionResponse` (inline reply) or a deferral (HTTP 202).
- Webhook events: PING (`type: 0`) gives 204. Events give 204 within 3 s and go to the handler.
- Adapters: `axum` and `hyper` (features). The core functions take bytes and headers, so other servers can use them.
- The framework can use this crate as an interaction source in place of the gateway.

## 11. Voice crate

### 11.1 Structure

```text
gateway (op 4) --> VOICE_STATE_UPDATE + VOICE_SERVER_UPDATE
                        |
                        v
+--------------------- Call (one per guild) ---------------------+
| voice WS task: v8 protocol, heartbeat, resume, DAVE messages   |
| UDP send task: 20 ms clock, RTP, DAVE encrypt, AEAD encrypt    |
| UDP receive task: AEAD decrypt, DAVE decrypt, jitter, Opus     |
| mixer: tracks, volume, queue                                   |
+----------------------------------------------------------------+
```

- The voice protocol uses a sans-IO core, like the gateway (section 7.1).
- Voice gateway version 8. Buffered resume with `seq_ack`.
- Transport encryption: `aead_aes256_gcm_rtpsize` when offered, else `aead_xchacha20_poly1305_rtpsize`. Use RustCrypto (`aes-gcm`, `chacha20poly1305`).
- The send task sends one packet each 20 ms. It sends 5 silence frames before a pause.

### 11.2 DAVE (end-to-end encryption)

- DAVE is mandatory for voice in normal channels since 2026-03-02 (close code 4017). Without DAVE, voice does not work.
- Implement DAVE in this project on `openmls` with ciphersuite 2 (`MLS_128_DHKEMP256_AES128GCM_SHA256_P256`).
- Put the DAVE logic behind a `DaveSession` trait. A second backend through FFI to Discord's `libdave` is then possible, if needed.
- Use `libdave` as a test reference: compare key packages, frame encryption, and ratchets with its output.
- Parts: MLS group handling (external sender, proposals, commits, welcome), transitions (ops 21..31), per-sender key ratchets, frame encrypt and decrypt (truncated tag, ULEB128 nonce, `0xFAFA` marker), and silence passthrough.

### 11.3 Audio

- Opus encode and decode through `libopus` (FFI). No mature pure-Rust Opus encoder exists.
- Inputs:
  - Raw PCM (`f32` or `i16`, 48 kHz, stereo).
  - Ogg Opus files. These go out without a new encode.
  - A decode feature with `symphonia` for common formats.
  - A process input (for example `ffmpeg`). It uses only the standard library, so it needs no feature.
- Seek: PCM in memory, Ogg Opus files, and the `symphonia` decode. Ogg Opus applies the pre-skip and the end trim when the mixer decodes it.
- Receive: a jitter buffer for each SSRC, Opus decode with loss concealment, and events with the user ID and PCM.
- Soundboard: an HTTP call, but the bot must be in the voice channel. The `Call` API gives a helper.

## 12. Testing

| Level | Method |
|---|---|
| Model | Round-trip tests on JSON fixtures. Use the examples in the Discord docs. |
| Model | Property tests: snowflakes, permissions, flags with unknown bits. |
| Model | Fuzz the deserializers (`cargo-fuzz`). Malformed input must give an error, not a panic. |
| HTTP | Fake transport. Scripted 429 responses with each scope. Bucket sharing. Header loss. |
| Gateway | Scripted frames into the sans-IO core with fake time: resume, op 7, op 9, zombie, close codes. |
| Gateway | Local mock gateway server for driver tests (compression, reconnect). |
| Cache | Event sequences give the expected state. Obfuscation cases. |
| Voice | Packet test vectors for both AEAD modes. DAVE checks against `libdave`. |
| Live | Tests against a real test guild. They need a secret token. Run them by hand or each night. |

### 12.1 Specification drift check

- Discord publishes an OpenAPI 3.1 specification in public preview (`github.com/discord/discord-api-spec`).
- `cargo xtask spec-check` compares the route templates and the object fields of the library with the specification.
- The report shows: endpoints not implemented, fields not modeled, and enum values not known.
- Reason: the hand-written types keep good names and documentation. The check finds what is missing. The specification is not stable, so it is not the source of generated code.

## 13. Versions and compatibility

- One version number for all workspace crates (see `AGENTS.md`).
- Major version 0 until the public release.
- Rust edition 2024.
- MSRV: 1.97 (latest stable minus 2 releases, section 15).
- The Discord API version is a constant. Version 10 now.

## 14. Feature flags of the facade crate

| Feature | Default | Effect |
|---|---|---|
| `http` | yes | HTTP client |
| `gateway` | yes | Gateway shards |
| `cache` | yes | In-memory cache |
| `framework` | yes | Router, commands, components |
| `macros` | yes | Derive macros |
| `endpoint` | no | HTTP interactions and webhook events |
| `voice` | no | Voice (brings `libopus`) |
| `zlib` | yes | `zlib-stream` gateway compression |
| `zstd` | no | `zstd-stream` gateway compression |
| `simd-json` | no | Faster JSON decode |
| `rustls` | yes | TLS with rustls |
| `native-tls` | no | TLS with the system library |
| `metrics` | no | Metrics counters |

## 15. Decisions

Decided on 2026-11-05:

| Topic | Decision |
|---|---|
| Project and crate names | `cordial`, `cordial-*` (free on crates.io on 2026-11-05) |
| License | MIT OR Apache-2.0 |
| MSRV | Latest stable minus 2 releases. Now 1.97. |

Open (recommendation given):

| Topic | Recommendation |
|---|---|
| ETF encoding | Not in version 1. |
| DAVE backend | Own implementation on `openmls`. `libdave` only as a test reference. |
| Social SDK endpoints (lobbies, identity profiles) | Include in HTTP coverage, low priority. |
| HTTP transport | `reqwest` behind the `Transport` trait. |
