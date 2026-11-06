# cordial

A full-featured Rust library for Discord bots.

> Status: pre-release. All roadmap phases are complete, except a live soak test with 1000+ guilds.
> Live tests with Discord ran for command sync, interactions, the gateway, and voice with DAVE.
> The public API can change before 1.0. See the [roadmap](docs/ROADMAP.md).

## AI-generated code

This project is vibecoded. Claude, an AI model from Anthropic, wrote most of the code, the tests, and the documents.
Read and test the code before you use it in production.

## Goals

- Cover the full bot surface of the Discord API: HTTP, gateway, interactions, voice (with DAVE), and OAuth2.
- Let a bot use only the layers that it needs.
- Accept new fields, enum values, and events from Discord without a break.
- Make the correct use the easy use: rate limits, intents, limits, and permissions.

## Crates

| Crate | Purpose |
|---|---|
| `cordial` | Facade. Re-exports the other crates through features. |
| `cordial-model` | Data types. No I/O. |
| `cordial-http` | HTTP client, rate limits, OAuth2. |
| `cordial-gateway` | Gateway shards and the shard manager. |
| `cordial-cache` | In-memory state. |
| `cordial-endpoint` | Interactions and webhook events over HTTP. |
| `cordial-voice` | Voice connections. |
| `cordial-macros` | Derive macros. |
| `cordial-framework` | Event router, commands, components. |

## Documents

- [Architecture](docs/ARCHITECTURE.md)
- [Roadmap](docs/ROADMAP.md)
- [Research notes on the Discord API](docs/research/README.md)
- [Contributing](CONTRIBUTING.md)

## License

Licensed under either of these, at your option:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

Unless you state otherwise, any contribution that you intentionally submit for inclusion in this work,
as defined in the Apache-2.0 license, is dual licensed as above, with no additional terms or conditions.
