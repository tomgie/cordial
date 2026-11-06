# Roadmap

Status: draft 1, 2026-11-05.

This roadmap lists all the work for a full bot library, in phases. Each phase has deliverables and exit criteria.
The structure is in [ARCHITECTURE.md](ARCHITECTURE.md). The facts are in [research/](research/).
Mark an item `[x]` in the same commit that completes it.

## Phase order

```text
P0 Foundation
 |
P1 Model core
 |
P2 HTTP core ------------------------------+
 |                                         |
P3 Gateway ------------+                   |
 |                     |                   |
P4 Interactions        P8 Voice (parallel) P7 Full HTTP coverage (parallel, after P2)
 |                     |                   |
P5 Cache               |                   |
 |                     |                   |
P6 Framework + macros  |                   |
 |                     |                   |
 +---------------------+-------------------+
 |
P9 Hardening and release
```

- P1 to P3 are the critical path. Everything else depends on them.
- P7 (the long list of endpoints) can run in parallel after P2. Its work is regular and repetitive.
- P8 (voice) can start after P3. It is the phase with the highest risk.

## P0: Foundation

Deliverables:

- [x] Cargo workspace with the crates in ARCHITECTURE section 3 (empty stubs).
- [x] Rust edition 2024. `rust-toolchain.toml`. MSRV in `Cargo.toml`.
- [x] Lints: `clippy` (pedantic subset), `rustfmt.toml`, `missing_docs = "deny"`, `unsafe_code = "forbid"` (FFI crates override it later).
- [x] CI: build, test, clippy, fmt, docs, MSRV check, feature combinations, `wasm32` model check, `cargo deny`, on Windows, Linux, and macOS.
- [x] License files. `README.md`. `CONTRIBUTING.md`.
- [x] `xtask` crate stub.
- [x] Discord docs examples as JSON fixtures under `fixtures/docs` (`cargo xtask fixtures`).

Exit: CI is green on an empty workspace.

## P1: Model core

Base types:

- [x] `Id<T>`, markers, snowflake time helpers, string and integer decode.
- [x] `Timestamp` (ISO 8601).
- [x] `discord_enum!` macro (integer and string enums with `Unknown`).
- [x] Bit flags type with 64 bits and unknown-bit preservation.
- [x] `Nullable<T>`, `Patch<T>`, null-means-true deserializer.
- [x] `Permissions` (bits 0..52) and the permission calculator with implicit rules.
- [x] `Color`, `ImageHash` (with the animated `a_` prefix), `Locale`.
- [x] CDN URL builder for each asset path in research/02-http.md section 6.
- [x] Message formatting helpers (mentions, timestamps with styles, emoji, `<@$GAME_ID>`).

Objects (see research/05-resources.md):

- [x] User, avatar decoration, collectibles, primary guild.
- [x] Guild, unavailable guild, guild preview, widget, welcome screen, onboarding, integration, ban, incidents data.
- [x] Member, partial member, member flags.
- [x] Role, role tags, role colors, partial role.
- [x] Channel (all 13 types), overwrites, thread metadata, thread member, forum tags, obfuscated flag.
- [x] Message, message types, flags, reference, snapshot, interaction metadata (union), reactions, activity, call, role subscription data, shared client theme.
- [x] Embed (with all flags), attachment, attachment request.
- [x] Emoji, sticker, sticker pack, soundboard sound.
- [x] Poll, poll request, poll results.
- [x] Invite, invite metadata, target-user job status.
- [x] Scheduled event, recurrence rule, event user.
- [x] Auto moderation rule, trigger metadata, actions.
- [x] Audit log, entry, change (typed accessors), optional info, all 71 action types.
- [x] Webhook, stage instance, guild template, voice state, voice region.
- [x] Application, application flags (with `flags_new`), install params, team.
- [x] SKU, entitlement, subscription.
- [x] Lobby, lobby member.

Exit: all docs examples decode and encode back without loss. Unknown values survive a round trip.

## P2: HTTP core

- [x] `Token` type (redacted, zeroed on drop).
- [x] `Endpoint` trait, `Route`, major parameter, `IntoFuture` request pattern.
- [x] `Transport` trait and the `reqwest` implementation. Fake transport for tests.
- [x] `User-Agent`, `Authorization`, `X-Audit-Log-Reason` (URL-encoded).
- [x] Request bodies: JSON, form, multipart with `payload_json` and `files[n]`.
- [x] Error decode: status, `JsonErrorCode` (245 codes), nested field errors.
- [x] Rate limiter: route keys, bucket discovery, temporary buckets, global limiter, 429 scopes, FIFO queues, idle bucket removal.
- [x] Invalid-request budget tracker. Token invalidation on 401.
- [x] Rate-limit JSON error codes as typed errors (20016, 20028, 20029, 30046, 40062, 110000).
- [x] Retry policy (idempotent methods, `enforce_nonce`).
- [x] Client-side validation framework (ARCHITECTURE section 5.4).
- [x] First endpoints: Get Gateway, Get Gateway Bot, Get Current User, Get Channel, Create Message (with files), Edit Message, Delete Message, Get Messages.
- [x] `xtask spec-check` first version: compare routes with the OpenAPI specification.

Exit: a test bot sends a message with an attachment. Rate-limit tests pass for all 429 scopes.

## P3: Gateway

- [x] Gateway payload types: opcodes 0..11, 31, 43. Identify (with `capabilities`), Resume, Presence, Voice State, Request Guild Members, Request Soundboard Sounds, Request Channel Info.
- [x] `Intents` (bits 0..25) and the event-to-intent table.
- [x] All dispatch event structs and the `Event` enum (see research/01-gateway.md section 7). Include RATE_LIMITED, CHANNEL_INFO, SOUNDBOARD_SOUNDS, VOICE_CHANNEL_STATUS_UPDATE, VOICE_CHANNEL_START_TIME_UPDATE.
- [x] `EventType` flags and two-step decode with an event filter.
- [x] Sans-IO `ShardCore`: Hello, Identify, heartbeat with jitter, ACK tracking, zombie detection, Resume, op 7, op 9, all close codes.
- [x] Close codes 4013 and 4014 give a fatal error that names the intents.
- [x] Shard driver with `tokio-tungstenite`.
- [x] `zlib-stream` decompression. `zstd-stream` behind a feature.
- [x] Send limiter (120 per 60 s, heartbeat reserve), 4096-byte check.
- [x] Op 8 limit (1 all-members request per guild per 30 s) and RATE_LIMITED handling.
- [x] Member chunk collector: `request_members(...)` gives a stream of chunks, matched by `nonce`.
- [x] `ShardManager`, `IdentifyQueue` trait, `LocalQueue`, session start limit guard.
- [x] Session export and import for resume after a process restart.
- [x] Channel obfuscation: handle the flag. Test with `capabilities: 1<<15`.
- [x] Mock gateway server for driver tests.

Exit (open: needs a bot token and a test guild): a test bot with 4 shards runs for 24 hours on a test guild. It resumes after forced disconnects. No event is lost (sequence check).

## P4: Interactions

Model:

- [x] Interaction object, types, contexts, integration types, `authorizing_integration_owners`.
- [x] Command data, resolved data, options (all 11 types), autocomplete focus.
- [x] Callback types 1..12 and response bodies. Callback response object (`with_response`).
- [x] Components: all 20 types (1..14, 17, 18, 19, 21, 22, 23) with `Unknown`. Unfurled media item, content scan flags.
- [x] Modal submit data shapes (Label, Text Display, all inputs).
- [x] Application command object, option object, choices, localizations, permissions.

HTTP:

- [x] Interaction callback, original response (get, edit, delete), follow-ups (create, get, edit, delete).
- [x] Application command endpoints (global and guild, bulk overwrite) and permission endpoints.

Builders and validation:

- [x] Message builder (V1 and V2), embed builder, component builders, modal builder, command builder.
- [x] Limits: 40 components, flag rules for IS_COMPONENTS_V2, 5 action rows, modal 1..5 children, command 8000 characters, name regex.

Endpoint crate:

- [x] Ed25519 verification, timestamp age check.
- [x] PING/PONG, inline reply, 202 deferral.
- [x] Webhook events: envelope, PING (204), all event types.
- [x] `axum` and `hyper` adapters.

Exit: one example bot answers commands over the gateway. A second example answers the same commands over HTTP. (Both examples build: `ping` and `http_interactions`. A live run needs a bot token; open.)

## P5: Cache

- [x] Resource selection and the required-event set.
- [x] Compact cached types. Shared users.
- [x] Update logic for every event that changes state. Return the previous value.
- [x] GUILD_CREATE fill, GUILD_DELETE (outage versus removal), unavailable guilds.
- [x] Obfuscated channels.
- [x] Optional message ring buffer. Reaction events change the cached messages.
- [x] Permission helpers on the cache.
- [x] Memory report: stack bytes per cached type (`cached_type_sizes` test). Heap measurement is in P9.

Exit: event sequence tests pass (done). A recorded event log of 1 hour replays without a panic (needs a bot token; open).

## P6: Framework and macros

- [x] `Bot` builder, `Context<S>`, typed state.
- [x] Event router with `on::<E>`, intent computation, privileged-intent warnings.
- [x] Intent check: missing intents, unused intents, privileged intents against application flags, `MESSAGE_CONTENT` warning (ARCHITECTURE 9.2).
- [x] Task model: one task per event, optional ordered mode per shard (thus per guild), panic isolation, error hook.
- [x] `#[derive(Command)]`, `#[derive(CommandGroup)]`, `#[derive(Choice)]`, `#[derive(Modal)]`.
- [x] Option types: primitives, resolved users, members, roles, channels, attachments, mentionables.
- [x] Autocomplete handlers.
- [x] Localization (`Localizer` trait).
- [x] Checks and cooldowns.
- [x] Command sync: normalize, compare by `(type, name)`, plan (create, update, rename, delete, unchanged).
- [x] Sync strategies: `Granular` (default), `Bulk`, `Auto`. Dry-run mode. Keep the Entry Point command.
- [ ] Normalization tests with real responses from Discord (defaults, null versus absent, localizations). The tests use responses in the documented shape; a capture from Discord needs a bot token.
- [x] Response state tracking, auto-defer option, late-acknowledge warning.
- [x] Component router with `custom_id` patterns.
- [x] Collectors (event, component, modal) with timeouts.
- [x] Interaction source: gateway or the endpoint crate.
- [x] Graceful shutdown with session export.
- [x] Examples: ping bot, moderation bot, poll bot, modal form, user-installable app, HTTP interactions.

Exit: the examples run. (They build; a live run needs a bot token.) The first public preview (`0.1.0`) is possible here, without voice and without full HTTP coverage.

## P7: Full HTTP coverage

All 245 operations in [research/endpoints.txt](research/endpoints.txt). Groups:

- [x] Application (3), application role connection metadata (2), application identity profile (5).
- [x] Audit log (1).
- [x] Auto moderation (5).
- [x] Channel (26): with permission endpoints, voice status, threads, followers, typing, group DM recipients.
- [x] Message (20): with search, new pin routes, reactions, bulk delete, crosspost. Old pin routes marked deprecated.
- [x] Poll (2).
- [x] Emoji (10): guild and application emojis.
- [x] Entitlement (5), SKU (1), subscription (2).
- [x] Guild (47): members, bans, bulk ban, roles, role member counts, permission endpoints, prune, widget, onboarding, welcome screen, incident actions, vanity URL.
- [x] Guild scheduled event (6).
- [x] Guild template (6).
- [x] Invite (9): with target users (CSV upload and download) and job status.
- [x] Lobby (16).
- [x] Soundboard (7).
- [x] Stage instance (4).
- [x] Sticker (8).
- [x] User (12): with the `shard` parameter on Get Current User Guilds for large bots.
- [x] Voice (5).
- [x] Webhook (15): execute with `wait`, `thread_id`, `with_components`; Slack and GitHub compatible.
- [x] OAuth2: authorization code (with PKCE), refresh, revoke, client credentials, bot and advanced bot authorization URLs, `iss` check, `GET /oauth2/@me`.
- [x] Pagination helpers: streams over messages, members, bans, audit log, entitlements, archived threads, reactions, pins.
- [x] `xtask spec-check` reports 0 missing endpoints.

Exit: the coverage report is complete. Each endpoint has at least one test with the fake transport.

## P8: Voice

Transport:

- [x] Voice payload types (ops 0..13, 21..31). Close codes.
- [x] Sans-IO voice core: Hello, Identify, Ready, Select Protocol, Session Description, heartbeat v8 with `seq_ack`, Resume, buffered resume.
- [x] Join and leave through the gateway (op 4, VOICE_STATE_UPDATE, VOICE_SERVER_UPDATE). Null endpoint handling. Channel moves.
- [x] UDP: IP discovery, RTP header, keepalive filter, RTCP filter.
- [x] UDP keepalive send, so that a bot that only receives keeps its NAT mapping.
- [x] AEAD modes: `aead_aes256_gcm_rtpsize`, `aead_xchacha20_poly1305_rtpsize`. Test vectors.

DAVE:

- [x] MLS group on `openmls`, ciphersuite 2, basic credentials, external sender.
- [x] Binary frame codec (TLS presentation language, MLS varints).
- [x] Key package, proposals (append and revoke), commit and welcome, announce commit, welcome.
- [x] Transitions: prepare, execute, ready, prepare epoch, invalid commit/welcome recovery.
- [x] Per-sender hash ratchet, generations, nonce wrap, 10 s retention of old keys.
- [x] Frame encryption and decryption: truncated tag, ULEB128 nonce, unencrypted ranges, supplemental size, `0xFAFA` marker, silence passthrough, replay rejection.
- [ ] Interoperability tests against `libdave`.
- [ ] Test in a real voice channel with Discord clients.

Audio:

- [x] Opus encode and decode through `libopus`.
- [x] Send clock (20 ms), speaking flags, silence frames.
- [x] Inputs: PCM, Ogg Opus passthrough, `symphonia` decode.
- [x] Process input (ffmpeg): `ProcessSource`. Ogg pre-skip and end trim. Opus packets shorter than 20 ms.
- [x] Mixer: many tracks, volume, pause, queue.
- [x] Seek: `TrackHandle::seek` for PCM in memory, Ogg Opus files, and `symphonia`.
- [x] Receive: SSRC-to-user map (ops 5, 11, 13), jitter buffer, loss concealment, PCM events.
- [x] Soundboard helper.
- [x] Stage helpers (suppress, request to speak). Voice channel status.

Exit: a music example plays a file in a DAVE call for 1 hour with Discord clients that listen. A recorder example saves each speaker to a separate file.

## P9: Hardening and release

- [ ] Fuzz targets for all decoders (gateway, HTTP responses, voice packets, DAVE frames).
- [ ] Benchmarks: event decode rate, cache memory, rate limiter overhead.
- [ ] Soak test: 1000+ guilds on a test application, or a recorded large-bot event log.
- [ ] Guide book (mdBook): start, intents, commands, components, sharding, cache, voice, HTTP interactions, deploy.
- [ ] Rustdoc for every public item, with an example on each main type.
- [ ] Public API review: names, `#[non_exhaustive]`, sealed traits, error types.
- [x] Security review: token handling, signature verification, input limits (`SECURITY.md`).
- [ ] Release process: changelog, version bump in all crates, publish order.

Exit: version `1.0.0` criteria are met (see below).

## Version 1.0 criteria

1. All P0..P9 items are done.
2. `spec-check` reports full endpoint coverage.
3. No known panic on malformed input.
4. The public API has been stable for two minor releases.
5. Each item in "Known API changes" below is handled.

## Known API changes to track

| Date | Change | Phase |
|---|---|---|
| 2026-11-16 | Channel obfuscation becomes mandatory. `GET /guilds/{id}/channels` omits hidden channels. | P1, P3, P5 |
| Future API version | `message_reference.type` required | P1 |
| Future API version | Start Thread without Message `type` required | P7 |
| Future API version | `enforce_nonce` is the default | P2 |
| No date | Old pin routes, role `color`, message `interaction`, `dm_permission`, PREMIUM_REQUIRED, Action Row in modals | Mark as deprecated |

Process:

- Check the Discord change log each week. The source is `developers/change-log.mdx` in `discord/discord-api-docs`.
- Run `xtask spec-check` in CI each week. A new difference opens an issue.

## Risks

| Risk | Effect | Mitigation |
|---|---|---|
| DAVE complexity | Voice is late or does not work | Start P8 early in parallel. Test against `libdave`. Keep a `DaveSession` trait for an FFI fallback. |
| API drift | Silent breaks | `Unknown` variants, lenient decode, weekly `spec-check`, change log watch. |
| Size of the endpoint list | Slow P7 | One `Endpoint` type per operation. A generator in `xtask` makes stubs from the specification. A person reviews and documents each one. |
| Rate-limit errors | Cloudflare ban of the IP | Strong tests with scripted 429 sequences. Invalid-request budget tracker. |
| Docs are not consistent | Wrong behavior | Open points in each research file. Live tests decide. |
| Privileged intent policy | Users cannot get intents | Document the policy. Make features work without privileged intents where possible. |

## Open questions from the research

These need a live test or a decision:

- Wait time before identify after op 9 with `d = false` (plan: 1..5 s at random).
- Per-opcode limits for gateway ops 31 and 43.
- Maximum attachments per message (plan: no client-side limit, rely on the server error).
- Text Display character limit in Components V2.
- Whether a bulk command overwrite fails when it omits the Entry Point command.
- Byte order of the DAVE truncated nonce in the GCM nonce.
- Whether JSON and binary voice messages share one sequence counter.
