# Intents

Intents are bit flags that the bot sends when it connects to the gateway. Each intent
selects a group of events. Discord sends only the events of the selected intents.

Examples:

| Intent | Events |
|---|---|
| `GUILDS` | Guild, channel, thread, role, and stage events |
| `GUILD_MEMBERS` | Member add, update, and remove |
| `GUILD_MESSAGES` | Messages in guilds |
| `GUILD_VOICE_STATES` | Voice state updates |
| `MESSAGE_CONTENT` | The content of messages (not a group of events) |

Interactions need no intent. A bot with only slash commands can use `Intents::empty()`.

## Privileged intents

Three intents are privileged:

| Intent | Application flag in the Developer Portal |
|---|---|
| `GUILD_MEMBERS` | Server Members Intent |
| `GUILD_PRESENCES` | Presence Intent |
| `MESSAGE_CONTENT` | Message Content Intent |

Turn them on in the Developer Portal, on the "Bot" page, under "Privileged Gateway
Intents". A bot in 100 or more servers must be verified, and Discord must approve each
privileged intent.

Without `MESSAGE_CONTENT`, these message fields are empty: content, embeds, attachments,
components, and poll. The fields are not empty in these cases:

- Direct messages to the bot.
- Messages that mention the bot.
- Messages of the bot.

## How the bot computes the intents

When you do not call `BotBuilder::intents`, the bot computes the intents. It uses these
facts:

1. The event types of the handlers that you add with `on`.
2. The event types that the cache needs for its resource types.
3. `INTERACTION_CREATE` when the bot has commands, components, or modals.
4. The application flags from `GET /applications/@me`.

The bot adds `MESSAGE_CONTENT` when a handler or the cache reads messages and the flag is
on in the Developer Portal. The bot removes a privileged intent that is not on, and logs a
warning for each event that it does not get.

```rust
{{#include ../../../crates/cordial/tests/guide/intents.rs:computed}}
```

A handler from `on_any` gets all events. Thus the bot requests all intents for it. Use
`on_any` only when the bot needs all events.

## Set the intents yourself

Call `BotBuilder::intents` to set the intents. The intent check still runs.

```rust
{{#include ../../../crates/cordial/tests/guide/intents.rs:explicit}}
```

## The intent check

The check runs before the bot connects.

| Problem | `Warn` (default) | `Strict` | `Off` |
|---|---|---|---|
| A handler or the cache needs an intent that is not set | Warning | Error | Nothing |
| An intent is set, but no handler uses it | Info | Info | Nothing |
| A privileged intent is set, but it is not on in the portal | Error | Error | Nothing |
| Message handlers without `MESSAGE_CONTENT` | Warning | Warning | Nothing |

Set the mode with `BotBuilder::intent_check(IntentCheck::Strict)`.

The first two rows apply only to intents that you set yourself. With computed intents,
nothing is missing and nothing is unused.

## Close codes 4013 and 4014

Discord closes the connection when the intents are wrong:

| Code | Meaning |
|---|---|
| 4013 | The intents are not valid (an unknown bit). |
| 4014 | The intents are not allowed: a privileged intent is not on or not approved. |

The shard does not connect again after these codes. `Bot::run` stops with
`Error::ShardFatal`. For code 4014, the message names the privileged intents that are not
on in the Developer Portal.

The intent check finds most of these problems before the bot connects. Code 4014 can
still occur when Discord removes an approval, or when you change the portal settings while
the bot runs.
