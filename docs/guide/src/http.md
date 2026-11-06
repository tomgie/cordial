# HTTP without the framework

The crate `cordial-http` (module `cordial::http`) is a complete client for the HTTP API of
Discord. Use it alone for scripts, web services, and tools. Turn off the default features
of `cordial` and turn on `http`:

```toml
[dependencies]
cordial = { version = "0.7", default-features = false, features = ["http"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

In the framework, `ctx.http()` gives the client of the bot.

## The client

`Client::new(token)` makes a client with the default settings. `Client::builder()` gives
more settings.

```rust
{{#include ../../../crates/cordial/tests/guide/http.rs:client}}
```

| Builder method | Default |
|---|---|
| `token` | none |
| `base_url` | `https://discord.com/api/v10` (change it for a rate limit proxy) |
| `user_agent_suffix` | none |
| `timeout` | 30 seconds for each request |
| `max_retries` | 3 retries after a server error or a connection error |
| `max_rate_limit_retries` | 5 retries after status 429 |
| `rate_limiter` | `InMemoryRateLimiter` |
| `transport` | `reqwest` |
| `without_validation` | the client-side checks are on |

The client is cheap to clone. All clones share the rate limiter and the connections.

## Endpoints

The client has one method for each endpoint of the Discord API. Each method gives a
request type with setters. Write `.await` to send the request.

```rust
{{#include ../../../crates/cordial/tests/guide/http.rs:send}}
```

The request types are in `cordial::http::endpoints`, in one module for each resource. For
example, `client.ban_member(guild_id, user_id)` gives a `BanMember` request. Many requests
have `.reason(...)`: the text goes to the audit log.

## Client-side checks

The client checks the documented limits before it sends a request. Examples: 2000
characters of content, 10 embeds, 25 choices, a `custom_id` of 1 to 100 characters. A
request that breaks a limit gives `Error::Validation` and does not go out.

The checks protect the bot from the invalid request limit. Discord counts the responses
401, 403, and 429. At 10,000 such responses in 10 minutes, Discord bans the IP address for
some time. The client warns at 5,000 and slows down at 9,000.

`without_validation` turns the checks off. Use it only when Discord changes a limit before
the library does.

## Errors

| `Error` variant | Cause |
|---|---|
| `Api(ApiError)` | Discord answered with an error status. |
| `Validation(error)` | A value breaks a documented limit. The request did not go out. |
| `RateLimited { route, retry_after, global }` | The client did not wait, or waited too often. |
| `InvalidToken` | An earlier request got status 401. The client stops all requests. |
| `MissingToken` | The request needs a token, but the client has none. |
| `Transport(error)` | The connection failed or timed out. |
| `Decode { .. }` | The response body does not match the expected type. |
| `Encode(error)` | The request body could not be encoded. |

`ApiError` has the HTTP status, the route, the `JsonErrorCode`, the message, and the field
errors. `Error::code()` gives the JSON error code directly. `JsonErrorCode` has a variant
for each documented code, for example `UnknownMessage` (10008) and `MissingPermissions`
(50013).

```rust
{{#include ../../../crates/cordial/tests/guide/http.rs:errors}}
```

## Rate limits

The default `InMemoryRateLimiter` follows the rate limit headers of Discord:

- It keeps the limit of each bucket, and the order of the requests in each bucket.
- It holds back requests to a new bucket until the first response tells the limit.
- It applies the global limit of 50 requests each second.
- After a 429 response, it waits for `retry_after` and sends the request again.

| Need | Setting |
|---|---|
| A bot with a higher global limit from Discord | `InMemoryRateLimiter::with_global_limit(n)` |
| A rate limit proxy in front of the bot | `base_url(...)` and `rate_limiter(NoRateLimiter)` |
| An error in place of a wait after 429 | `max_rate_limit_retries(0)` |
| Your own limiter, for example in Redis | Implement the `RateLimiter` trait |

Interaction responses and follow-up messages do not count against the global limit.

## Pagination

Some lists come in pages. The client has helpers that read all pages with a cursor. Each
helper gives a `Pages` value. Call `next_page` until it gives `None`.

| Method | Reads |
|---|---|
| `message_history(channel_id)` | The messages of a channel, from the newest |
| `all_guild_members(guild_id)` | The members of a guild |
| `all_guild_bans(guild_id)` | The bans of a guild |
| `all_reactions(...)` | The users that reacted with an emoji |
| `all_current_user_guilds()` | The guilds of the bot |
| `all_entitlements(...)` | The entitlements of the application |
| `audit_log_history(...)` | The audit log entries |

```rust
{{#include ../../../crates/cordial/tests/guide/http.rs:pages}}
```

After an error, the next call of `next_page` tries the same page again.

## Files

`FileUpload::new(filename, bytes)` makes a file. Add it to a message with `.file(...)`.
The client sends a multipart request when a request has files.

- Refer to a file in an embed as `attachment://<filename>`.
- `content_type` sets the media type, for example `audio/ogg` for a voice message.
- The `reply` and `followup` methods of the framework cannot carry files. For a file in a
  follow-up message, call `ctx.http().create_followup(...)` with `.file(...)`. The
  interaction token is `ctx.interaction().token.expose()`.

Images for icons, avatars, banners, and emojis use `ImageData`, a data URI.
`ImageData::from_bytes` finds the media type from the first bytes.
