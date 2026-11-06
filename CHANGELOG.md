# Changelog

All crates of the workspace use one version. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Before 1.0.0, a minor version can break the public API.

## 0.12.1

### Changed

- `README.md`: the status line now shows the pre-release state. It said "Nothing works yet" since the first commit.
- `README.md`: a note tells that the project is vibecoded, with Claude as the main author of the code.

## 0.12.0

### Added

- `E2eeSession::can_encrypt`: tells if the session can encrypt a frame now. The default is `true`, so other implementations do not change.
- Example `live_voice` plays an audio file at a target loudness (default -22 LUFS). It measures the file first, and keeps the true peak at or below -1 dBTP. A list of targets, such as `-16,-22,-28`, changes the level each 20 seconds for an A/B comparison. The feature is now `symphonia`.

### Fixed

- The first frame of a track was lost when the track started before the first DAVE key. The send task now waits for `can_encrypt` before it takes a frame from the mixer, so the track starts at its first frame.
- Example `live_voice`: the bot counted its own voice state from an earlier run as a person in the channel, so it did not join.

## 0.11.4

### Added

- Example `live_voice` measures loudness (EBU R128) with the `ebur128` crate: the short-term loudness of each person in each status line, and the integrated loudness and true peak at the end. The first measurement of a person in a call gave -22.0 LUFS.

## 0.11.3

### Added

- Example `live_voice` (feature `opus`): a live test of a voice call with DAVE. The bot joins a voice channel only when one person is alone in it, plays a test tone, and shows the level of the received audio. The first run in a real channel passed in both directions.

## 0.11.2

### Added

- Crate `libdave-interop` (`interop/libdave`, not published): interoperability tests of the DAVE session against Discord's libdave. The tests load the libdave shared library at run time. Each library accepts the commits and Welcomes of the other, and frames pass both ways.
- `cordial-voice` feature `test-gateway` (internal): a test stand-in for the DAVE external sender of the voice gateway. It is not part of the public API.

## 0.11.1

### Added

- Example `live_bot`: a live test of the interactions in one guild. A person uses 12 test commands. The guild commands of other tools stay, because the sync includes them from a keep file.

## 0.11.0

Tools for live tests of the gateway.

### Added

- `Lifecycle::SequenceGap`: a dispatch skipped sequence numbers in one session, so events are lost. The shard also writes a warning.
- `Event::encode`: the inverse of `Event::decode`, for example to record and replay events.
- Example `live_gateway`: connect, fill the cache, request members, and resume the session in a new shard manager.
- Example `live_soak`: a long run with forced disconnects, a sequence check, a round-trip check of each event, and an event log. Its `replay` mode decodes a log into a new cache.

## 0.10.3

Results of the first live tests against Discord.

### Added

- Example `live_check`: a read-only check of a bot token (user, application, gateway, guilds, global commands).
- Example `live_sync`: a live test of the command sync in one guild (create, update, rename, bulk overwrite, delete). After each change, a second sync must find no changes.
- Sync tests with real command lists from Discord (`crates/cordial-framework/tests/sync_fixtures.rs`).

### Fixed

- Command sync: a guild command with `contexts` or `integration_types` no longer shows an update at each sync. Discord drops these fields from guild commands, so the comparison ignores them for a guild command. The sync writes a warning for such a definition.

## 0.10.2

Fixes for the problems that a review of the gateway and HTTP crates found.

### Fixed

- Rate limiter: after the local reset of a bucket, the limiter keeps an estimated reset time. Before, an empty bucket sent the next request at once and got a 429. This also occurred after each 429.
- Rate limiter: the first response after the local reset counts the requests that are in flight. A late response of an earlier window does not change the bucket.
- Rate limiter: a request takes its global slot after the wait for its bucket, not before. Before, many requests to empty buckets went out together and got a global 429.
- Rate limiter: the clean-up removes idle buckets after Discord gives the bucket hash. Before, the first bucket of each route stayed in memory.
- Gateway: a shard with queued commands and no active session does not wake up again and again (high CPU load).
- Gateway: `ShardHandle::send`, `request_members`, and `close` answer while the shard connects or waits to connect again. Commands wait in the queue and go out when the session is active. `close` stops the shard at once.
- Gateway: handle calls from the task that reads the events do not deadlock when the event channel is full.
- Gateway: a failed send reconnects one time. Commands that did not go out stay in the queue for the next session. Before, extra reconnects closed the next connection, and the commands were lost.
- Gateway: when Hello does not arrive within 20 seconds, the shard connects again.
- Gateway: `MemberChunks` no longer stops the shard when the caller does not read the chunks. `MemberChunks::next` gives `None` after a new session, and when no chunk arrives within 60 seconds.
- Gateway: `ShardManagerBuilder::large_threshold` clamps the value to 50 to 250, as `ShardConfig::large_threshold` does.

## 0.10.1

Fixes in `cordial-voice` from a review of the voice connection.

### Security

- A second Session Description (op 4) with the same key started a new transport cipher at nonce 0. The nonces then repeated with one key (AES-GCM nonce reuse). The core now accepts op 4 (and op 2 Ready) only in the handshake. The driver also continues the nonce counter when a new session gets a key that it used before.

### Fixed

- The manager kept a stopped call, and each later join failed. A join now replaces a stopped call. A stopped call keeps the `Stopped` state; a server update does not change it.
- A close between Ready and Session Description gave a resume that reported `Ready` with no media. The core now resumes only after a Session Description, and ignores a Resumed that it did not ask for.
- Resumes had no delay and no limit. A resume that does not reach Resumed now waits (0 s, 1 s, 2 s, 4 s, 8 s). After 5 such attempts, the core starts a new session.
- A leave while a join waited made a call for a channel that the bot left. The join now gives `JoinError::Left`.
- A join that timed out left a call that tried to connect for ever. The join now stops the call that it started and removes it from the manager.
- A stop or a new server during a connection attempt waited for the connect timeout (10 s). The driver now handles commands during the attempt.
- The SSRC-to-user map kept the users of an old session. A new session clears it.
- A jitter buffer with fewer packets than its depth waited for ever, and its old packets played before the next audio of the user. Playout now starts after `2 × depth` steps. The jitter buffer of a user who leaves is dropped.
- A voice server move (null endpoint) during the wait after a failed connection was ignored, and the call connected to the old server.
- When `VOICE_SERVER_UPDATE` came before `VOICE_STATE_UPDATE` in a move, the call kept the old channel ID. DAVE uses it as the group ID. A channel change now starts a new session (`ConnectionInfo::needs_new_session` compares the channel).
- The Ogg reader had no limit for a packet. An audio packet larger than 61,440 bytes (RFC 7845) is now a format error. A granule position near `i64::MAX` caused an overflow.

## 0.10.0

Fixes for the problems that the guide work found.

### Added

- `BotBuilder::voice` and `Context::voice`: the bot makes a voice manager with DAVE when the shards start, and gives it the voice events.
- `cordial_voice::ShardHandles`: shard handles with the total shard count of the bot.
- `VoiceManager::without_e2ee`, for tests with a local voice server.
- `BotBuilder::gateway`: change the shard manager settings, for example a shared identify queue, the compression, or the large threshold.
- `Reply::file`: a reply with files, for each response state.
- The facade features `opus` and `symphonia`.
- A clear message for close code 4013 (invalid intents).

### Changed

- `VoiceManager::new` uses DAVE end-to-end encryption. Discord needs it.
- `BotBuilder::autocomplete::<C>(handler)` needs no placeholders for the handler type (new trait `AutocompleteHandler`).
- `BotBuilder::cache_messages` adds the `MESSAGE` resource to the cache.

### Removed

- `impl VoiceGateway for Vec<ShardHandle>`. It used the number of handles as the shard count, which is wrong when a process runs only some shards. Use `ShardHandles`.

### Fixed

- The derive macros work in the doctests of `cordial-framework` and `cordial` without a `crate = ...` attribute.

## 0.9.2

### Fixed

- The rate limiter panicked on a very large wait time in a response header (`Instant` overflow). Waits now have a limit of 30 years. Found while writing the fuzz targets.

### Added

- Fuzz targets for all decoders (`fuzz/`), with a stable smoke test in CI.
- Benchmarks with criterion, and `docs/BENCHMARKS.md`.

## 0.9.1

Documentation only. The library behavior does not change.

### Added

- The guide book in `docs/guide` (mdBook): start, intents, commands, responses, components and modals, events and state, cache, HTTP, sharding, voice, HTTP interactions, and deploy. CI builds it.
- The test crate `crates/cordial/tests/guide`: it compiles the code of the guide.
- A crate-level example in each crate, and an example on each main type: `Client`, `ShardManager`, `Shard`, `Cache`, `Bot`, `BotBuilder`, `InteractionContext`, `Endpoint`, `VoiceManager`, `Call`, `Pattern`, `Command`, and `Modal`.

### Changed

- The voice examples use DAVE (`VoiceManager::with_e2ee`), because Discord needs it.

## 0.9.0

Fixes from a review of the framework and the cache.

### Fixed

- A long cooldown panicked on Windows at the first use (an `Instant` underflow).
- An automatic public defer made a later ephemeral reply public. The reply now replaces the loading message with an ephemeral follow-up.
- A panic in a collector filter stopped the bot. Now it stops only that collector. Filters run without the collector lock, so a filter can start or stop a collector.
- A handler that failed after a defer left the user with "thinking" for 15 minutes. The default error response now fits the response state. A failed autocomplete gets empty choices.
- A panic in a check skipped the error handling.
- `Context::shard_for_guild` used the shard count of the process, not of the bot.
- Interactions over HTTP skipped the collectors.
- An interaction that arrived as a collector timed out was lost.
- The automatic defer of a modal from a slash command used a type that Discord rejects.
- `update_message` after a reply edited the reply. It now gives `ResponseError::NotComponentMessage`.
- The automatic defer ran for interactions with no handler.
- Cache: a channel delete now removes its threads. `THREAD_LIST_SYNC` removes threads that are no longer active. A new `GUILD_CREATE` replaces the channels, threads, and roles. `READY` removes the guilds that the bot left while offline. A member removal removes the voice state.
- Cache: message buffers of deleted channels and removed guilds are removed also without the `CHANNEL` resource, and the number of buffers has a limit (10,000).

### Changed

- Cache: an outage (`GUILD_DELETE` with `unavailable`) keeps the guild data. Only a removal deletes it.

### Added

- `BotBuilder::on_interaction_error`: a hook that gets the interaction context and can answer the user.
- `BotBuilder::extra_events`: request event types for collectors. A collector of a type that the bot does not request logs a warning.
- `BotBuilder::sync_guilds`: sync guild scopes that have no commands now, to remove old guild commands.
- `ResponseState::Updated`.

## 0.8.1

### Added

- A synthetic soak test of the cache: many guilds, a random event stream, a shadow model, and a leak check.

## 0.8.0

### Changed

- `InteractionContext::get_reply` is now `InteractionContext::original_response`. The Rust API guidelines do not use a `get_` prefix.

### Added

- `cargo xtask release-check`: checks the versions, the crate metadata, and this changelog, and prints the publish order.
- `RELEASING.md`: the release steps.
- A compile-time test of the public API: error types are `Error + Send + Sync`, and handles are `Send + Sync`.

## 0.7.0

### Security

- Token fields use `cordial_model::Secret`. Its `Debug` hides the text. Before, the `Debug` of `Identify` showed the bot token.
- Transport errors drop the URL, and the `Debug` of `Route` hides paths that hold a token.
- A gateway message has a limit of 256 MiB, compressed or decompressed.

### Changed

- `Interaction::token`, `Webhook::token`, `VoiceServerUpdate::token`, `Identify::token`, and `Resume::token` have the type `Secret`. Read the text with `expose()`.

## 0.6.0

### Added

- `ProcessSource`: audio from `ffmpeg` or any program that writes PCM.
- `TrackHandle::seek` and `TrackHandle::position`.
- Ogg Opus pre-skip and end trim when the mixer decodes.

### Changed

- `SourceFrame::Opus` has the fields `discard_start` and `discard_end`.

## 0.5.1

### Changed

- DAVE uses an own OpenMLS provider for ciphersuite 2 (`hpke` 0.15, `p256` 0.14). This removes a timing advisory of `crypto-bigint` 0.5 and the MPL-2.0 HPKE crates.

## 0.5.0

### Added

- A UDP keepalive for voice connections that only receive.
- `Call::request_to_speak`, `Call::set_suppressed`, and `Call::set_channel_status`.

## 0.4.0

### Added

- Six example bots in `crates/cordial/examples`.
- The `axum` feature of the facade crate.
- `User::avatar_url`.

## 0.3.0

### Added

- `cordial-framework`: the bot builder, the event router, the intent check, interaction responses with state tracking, the component and modal router, collectors, checks, cooldowns, localization, and the granular command sync.
- `cordial-macros`: `Command`, `CommandGroup`, `Choice`, and `Modal` derive macros.
- `cordial-voice`: the voice gateway, the UDP transport, the AEAD modes, Opus, the mixer, receive, and DAVE end-to-end encryption.

### Fixed

- A command edit sends `nsfw` and the options, so that it can clear them.

## 0.2.0

### Added

- `cordial-cache`: selected resources, previous values, shared users, permissions, obfuscated channels, and a message ring buffer.
- Reaction helpers on `Message`.

## 0.1.0

### Added

- `cordial-model`: all Discord objects, events, permissions, CDN URLs, and validation.
- `cordial-http`: all endpoints of the OpenAPI specification, rate limits, retries, OAuth2, and pagination.
- `cordial-gateway`: the sans-IO shard core, the shard driver, compression, the identify queue, and the shard manager.
- `cordial-endpoint`: interactions and webhook events over HTTP with Ed25519 checks.
