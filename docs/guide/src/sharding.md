# Sharding

A shard is one gateway connection. Each shard gets the events of a part of the guilds.
Discord needs more shards when a bot is in more guilds: a shard can have at most 2,500
guilds.

The shard of a guild is `(guild_id >> 22) % shard_count`. The function
`cordial::gateway::shard_id_for_guild` computes it. All events of one guild come from one
shard.

## Shard ranges

`ShardRange` selects the shards of a process.

| `ShardRange` | Shards |
|---|---|
| `Recommended` (default) | All shards, with the count that Discord recommends |
| `All { count }` | All shards of a fixed count |
| `Some { ids, count }` | Some shards of a fixed count, for a bot in many processes |

The framework uses `ShardRange::Recommended`. Change it with `BotBuilder::shards`.

## The shard manager

Without the framework, `ShardManager` runs the shards and merges their events. It uses
the HTTP client for `GET /gateway/bot`.

```rust
{{#include ../../../crates/cordial/tests/guide/sharding.rs:manager}}
```

| Builder method | Use |
|---|---|
| `shards(range)` | The shards of this process. |
| `compression(compression)` | The transport compression. |
| `presence(presence)` | The presence at identify. |
| `event_types(types)` | The event types to decode. Other events are dropped without decode. |
| `identify_queue(queue)` | The identify queue. See below. |
| `resume_sessions(sessions)` | Sessions to resume, from `close_resumable`. |
| `large_threshold(n)` | The member count above which Discord does not send offline members. |
| `event_buffer(n)` | The capacity of the event channel. |

`ShardManager::shards` gives a `ShardHandle` for each shard. A handle sends gateway
commands (`send`, `request_members`), gives the latency and the session, and closes the
shard.

For one connection only, `Shard::start(ShardConfig::new(token, intents))` starts one
shard.

## The identify queue

Discord limits how fast shards can identify. Each application has a `max_concurrency`
value from `GET /gateway/bot`:

- The shards are in `max_concurrency` buckets. The bucket of a shard is
  `shard_id % max_concurrency`.
- Each bucket permits one identify every 5 seconds.

The default `LocalIdentifyQueue` applies these rules to the shards of one process. A bot
in many processes needs a shared queue, for example in Redis. Implement the
`IdentifyQueue` trait: its `acquire` method waits until the shard can identify. Give the
queue to `ShardManagerBuilder::identify_queue`.

## Multi-process bots

Split the shards between processes with `ShardRange::Some`. Each process must use the
same total count.

```rust
{{#include ../../../crates/cordial/tests/guide/sharding.rs:process}}
```

Things to know:

- Each process has its own cache, collectors, and state. Data that all processes need must
  be in an external store.
- The framework uses a local identify queue in each process. To share one queue between
  the processes, set it with `BotBuilder::gateway(|builder| builder.identify_queue(queue))`.
  `gateway` also sets the compression, the large threshold, and the event buffer.
- Interactions arrive on the shard of their guild. A DM interaction arrives on shard 0.

## Session start limits

Discord limits the number of new sessions each day (often 1,000). Discord resets the token
of a bot that goes over this limit.

At the start, the manager checks the remaining session starts. It keeps a reserve of
`SESSION_START_RESERVE` (5) starts. When too few starts remain, `start` gives
`ManagerError::SessionStartLimit` with the time until the reset.

A resumed session needs no new session start. Use resumable shutdown for restarts. See
[Deploy](deploy.md#resume-sessions-across-restarts).
