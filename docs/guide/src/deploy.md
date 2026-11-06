# Deploy

This chapter explains how to run a bot in production.

## Environment variables

Keep secrets out of the source code and out of the version control. Read them from the
environment at the start.

| Variable | Use | Read with |
|---|---|---|
| `DISCORD_TOKEN` | The bot token | `Token::from_env("DISCORD_TOKEN")` |
| `DISCORD_PUBLIC_KEY` | The public key, for HTTP interactions | `PublicKey::from_hex(&std::env::var(...)?)` |
| `RUST_LOG` | The log levels | `EnvFilter::from_default_env()` |

The names are conventions of the examples. Use the names of your platform when it has a
secret store.

## Logs with `tracing`

The library writes all logs with `tracing`. Start a subscriber in `main`, or you see no
logs.

| Level | Examples |
|---|---|
| `error` | A handler failed, a shard stopped. |
| `warn` | A missing intent, a slow interaction response, a rate limit, an event that does not decode. |
| `info` | The bot started, the command sync summary, the shard count. |
| `debug` | Each gateway event, each HTTP request, the stderr lines of an ffmpeg process. |

The targets are the crate names, for example `cordial_http` and `cordial_gateway`. With
`EnvFilter`, `RUST_LOG=info,cordial_http=debug` shows the HTTP requests and the `info`
logs of all other crates.

`Token`, the interaction tokens, and the voice tokens hide their text in their `Debug`
output. Thus a `?` value in a log line does not show a secret.

## Graceful shutdown

The bot stops at Ctrl+C, at `Context::shutdown`, or at a fatal gateway error. Then `run`
closes the shards and returns.

A container platform stops a process with SIGTERM. The bot listens for Ctrl+C (SIGINT)
only. To stop on SIGTERM, turn off the Ctrl+C handler with `stop_on_ctrl_c(false)`, wait
for the signal in a task, and call `shutdown` on a context from `Bot::context`.

Handlers that run at the time of the stop are not awaited. Keep important writes short, or
write them before the reply.

## Resume sessions across restarts

A restart normally starts new gateway sessions. The bot misses the events between the stop
and the start, and each new session counts against the daily session start limit.

With `ShutdownMode::Resumable`, the bot keeps the sessions open for a short time and gives
them back. The next process resumes them with `resume_sessions`. Discord then sends the
missed events. `Session` implements `Serialize` and `Deserialize`, so a file or a database
can keep it.

```rust
{{#include ../../../crates/cordial/tests/guide/deploy.rs:resume}}
```

Discord keeps a session open for a short time only. Start the new process soon. When a
resume fails, the shard identifies again with a new session.

## Rate limits

- Use one `Client` for all requests of a process. Clones share the rate limiter.
- Many processes with one token share the global limit of 50 requests each second. Put a
  rate limit proxy in front of them, and give each client `NoRateLimiter`.
- Watch `Client::invalid_request_count`. At 10,000 invalid requests in 10 minutes, Discord
  bans the IP address for some time.
- Keep the client-side checks on. They stop bad requests before they count.
- A restart loop can use all session starts of the day. Then Discord resets the token.
  The shard manager keeps a reserve and gives `ManagerError::SessionStartLimit` first.

## A checklist

1. Read the token from the environment.
2. Start a `tracing` subscriber with `RUST_LOG`.
3. Turn on only the privileged intents that the bot needs, in the portal and in the code.
4. Set an error hook that reports errors to your monitoring.
5. Use `ShutdownMode::Resumable` and keep the sessions between restarts.
6. Test new commands in a test guild with `CommandSettings::guilds` before you make them
   global.
