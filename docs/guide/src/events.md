# Events and state

The gateway sends events: a new message, a member that joins, a guild that becomes
available. The framework gives each event to the cache, to the collectors, and to the
event handlers.

## Event handlers

| Method | Handler signature | Gets |
|---|---|---|
| `on(handler)` | `async fn(Context<S>, E)` | Each event of type `E` |
| `on_any(handler)` | `async fn(Context<S>, Arc<Event>)` | All events |

The event types are in `cordial::model::gateway::event`, for example `MessageCreate`,
`GuildCreate`, and `Ready`. The bot finds the type `E` from the second argument of the
handler. Many event types dereference to their object: a `MessageCreate` dereferences to a
`Message`.

A handler returns `()` or `Result<(), E>`. Each event handler adds its event type to the
intents and to the event filter of the bot. See [Intents](intents.md).

## Typed state

The bot has one state value of your type. Each handler gets it with `ctx.state()`. Use
types with interior mutability for values that change, for example atomics,
`std::sync::Mutex`, `tokio::sync::Mutex`, or `DashMap`.

Call `BotBuilder::state` before you add handlers, because the handlers use the state type.
`build` gives an error when you call `state` after a handler.

```rust
{{#include ../../../crates/cordial/tests/guide/events.rs:state}}
```

## The context

`Context<S>` is cheap to clone. All clones share the same data.

| Method | Gives |
|---|---|
| `http()` | The HTTP client. |
| `cache()` | The cache, when the bot has one. |
| `state()` | The user state. |
| `application_id()` | The application ID. |
| `shard(id)`, `shards()`, `shard_for_guild(guild_id)` | The shard handles. |
| `wait_for`, `collect` | Collectors. See [Components and modals](components.md#collectors). |
| `shutdown()` | Stops the bot. |

`Bot::context` gives a context before `run`, for example to start a task.

## The error hook

The error hook gets the errors and the panics of all handlers. A panic in a handler does
not stop the bot.

```rust
{{#include ../../../crates/cordial/tests/guide/events.rs:error_hook}}
```

| `HandlerErrorKind` | Cause |
|---|---|
| `Error(error)` | The handler gave an error. |
| `Panic(message)` | The handler panicked. |
| `Parse(error)` | The interaction data does not match the command definition. Sync the commands. |

Without a hook, the bot logs the error. For an interaction without a response, it also
sends an ephemeral "An error occurred." message. With a hook, the bot does not send this
message. Send your own message from the hook when you need one.

## Event order

For each event, the bot does these steps:

1. It updates the cache.
2. It gives the event to the collectors.
3. For an interaction that no collector took, it starts the interaction handler in a new
   task.
4. It runs the event handlers: the `on_any` handlers first, then the `on` handlers, one
   after the other.

`EventOrder` sets how the events run:

| `EventOrder` | Behavior |
|---|---|
| `Concurrent` (default) | Each event runs in a new task. Handlers of different events run at the same time. |
| `PerShard` | The events of each shard run one after the other, in the order of arrival. |

All events of one guild come from one shard. Thus `PerShard` keeps the order of the events
of each guild. A slow handler holds back the next events of its shard. Interactions always
run at the same time, because Discord needs a response within 3 seconds.

## Shutdown

The bot stops in these cases:

- Ctrl+C. Turn this off with `stop_on_ctrl_c(false)`.
- A call of `Context::shutdown`.
- A fatal gateway error, for example close code 4014. `run` gives an error.

```rust
{{#include ../../../crates/cordial/tests/guide/events.rs:shutdown}}
```

The complete builder:

```rust
{{#include ../../../crates/cordial/tests/guide/events.rs:builder}}
```

`ShutdownMode` sets what happens to the gateway sessions:

| `ShutdownMode` | Effect |
|---|---|
| `End` (default) | The sessions end. The bot goes offline. |
| `Resumable` | The sessions stay open for a short time. `RunOutcome::sessions` gives them. |

Give the sessions to the next process with `BotBuilder::resume_sessions`. The new process
resumes them and gets the events that it missed. See [Deploy](deploy.md#resume-sessions-across-restarts).
