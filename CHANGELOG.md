# Changelog

All crates of the workspace use one version. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Before 1.0.0, a minor version can break the public API.

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
