# Changelog

All crates of the workspace use one version. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Before 1.0.0, a minor version can break the public API.

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
