# Introduction

cordial is a library for Discord bots in Rust. It contains all parts that a bot needs:

- The HTTP API, with one request type for each endpoint and a rate limiter.
- The gateway, with shards and a shard manager.
- A cache of guilds, channels, members, and other resources.
- A framework with typed commands, components, modals, and event handlers.
- Interactions over HTTP, without a gateway connection.
- Voice, with DAVE end-to-end encryption.

This guide shows how to use these parts. The API documentation (`cargo doc --open`) gives
the details of each type.

## The crates

cordial is a set of crates. Each crate is one layer. A layer uses only the layers below it.

| Crate | Contents |
|---|---|
| `cordial-model` | Data types: IDs, objects, events, permissions, builders. No I/O. |
| `cordial-http` | The HTTP client, the rate limiter, OAuth2. |
| `cordial-gateway` | Shards, the shard manager, the identify queue. |
| `cordial-cache` | The in-memory cache. |
| `cordial-endpoint` | Interactions and webhook events over HTTP. |
| `cordial-voice` | Voice connections, DAVE, audio sources, the mixer. |
| `cordial-macros` | The derive macros: `Command`, `CommandGroup`, `Choice`, `Modal`. |
| `cordial-framework` | The bot builder, the event router, commands, components. |
| `cordial` | The facade. It re-exports the other crates. |

Most bots use only the facade crate `cordial`.

## Feature flags of `cordial`

The facade crate turns on the layers with Cargo features.

| Feature | Module | Default | Turns on |
|---|---|---|---|
| (always) | `cordial::model` | yes | |
| `http` | `cordial::http` | yes | |
| `gateway` | `cordial::gateway` | yes | `http` |
| `cache` | `cordial::cache` | yes | |
| `framework` | `cordial::framework` | yes | `http`, `gateway`, `cache` |
| `macros` | `cordial::macros` | yes | |
| `endpoint` | `cordial::endpoint` | no | the HTTP interaction source of the framework |
| `axum` | `cordial::endpoint::axum` | no | `endpoint` |
| `voice` | `cordial::voice` | no | `gateway` |

The framework re-exports the derive macros, so `cordial::framework::Command` is the trait
and the derive macro.

A bot with the framework needs this in `Cargo.toml`:

```toml
[dependencies]
cordial = "0.7"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
tracing-subscriber = "0.3"
```

A bot that uses only the HTTP API turns off the default features:

```toml
[dependencies]
cordial = { version = "0.7", default-features = false, features = ["http"] }
```

The `opus` and `symphonia` features are features of `cordial-voice`, not of the facade.
See [Voice](voice.md).

## How to read this guide

The chapters build on each other:

1. [Start](start.md) makes a first bot.
2. [Intents](intents.md), [Commands](commands.md), [Responses](responses.md), and
   [Components and modals](components.md) explain the framework.
3. [Events and state](events.md) and [Cache](cache.md) explain the data of the bot.
4. [HTTP](http.md), [Sharding](sharding.md), [Voice](voice.md), and
   [HTTP interactions](http-interactions.md) explain the lower layers.
5. [Deploy](deploy.md) explains how to run a bot in production.

The code in this guide comes from the examples in `crates/cordial/examples` and from the
test crate `crates/cordial/tests/guide`. The tests make sure that the code compiles.
