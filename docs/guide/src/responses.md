# Responses

Discord needs one initial response to each interaction within 3 seconds. After that, the
bot can edit the original response and send follow-up messages for 15 minutes.

`InteractionContext` keeps the response state of the interaction. Thus `reply` always
sends the correct request. `CommandContext` and `ComponentContext` give the same methods.

## The methods

| Method | Use |
|---|---|
| `reply(message)` | Sends a message. The request depends on the state (see the table below). |
| `defer()` | Acknowledges now. The user sees "thinking" until the next reply. |
| `defer_ephemeral()` | The same, but the coming message is ephemeral. |
| `edit_reply(message)` | Edits the original response. |
| `followup(message)` | Sends a follow-up message. The interaction needs an initial response first. |
| `delete_reply()` | Deletes the original response. |
| `original_response()` | Gets the original response. |
| `modal(modal)` | Shows a modal. Only as the initial response. |
| `update_message(message)` | Components and modals: edits the message of the component. |
| `defer_update()` | Components and modals: acknowledges now, edits the message later. |
| `response_state()` | Gives the state. |

`reply` returns a `Reply`. Call `.ephemeral()` on it, and then `.await` it to send it.

A message is anything that converts into `InteractionMessage`: a `&str`, a `String`, an
`Embed`, or an `InteractionMessage` that you build.

## The response state

| State | Meaning | Request of `reply` |
|---|---|---|
| `Pending` | No response yet. | The initial response with the message |
| `Deferred { ephemeral }` | The bot deferred a message. | An edit of the original response (see below) |
| `DeferredUpdate` | The bot deferred an update of the component message. | A follow-up message |
| `Updated` | The bot updated the component message. | A follow-up message |
| `Responded` | The bot sent the initial response. | A follow-up message |

A defer sets the visibility of the loading message, and an edit cannot change it. Thus,
after a defer, `reply` compares the visibility of the message with the defer:

- The same visibility: `reply` edits the loading message.
- A different visibility: `reply` deletes the loading message and sends a follow-up
  message. For example, an ephemeral reply after a public defer stays private.

`update_message` works only in the states `Pending`, `DeferredUpdate`, and `Updated`. In
the other states, the original response is not the component message, so
`update_message` gives `ResponseError::NotComponentMessage`.

## Defer a slow command

```rust
{{#include ../../../crates/cordial/tests/guide/responses.rs:defer}}
```

`defer` does nothing when the interaction already has an initial response. `modal` gives
`ResponseError::AlreadyAcknowledged` in that case, because a modal is possible only as the
initial response.

## The slow response policy

A timer starts with each interaction. When the handler has not responded after the delay,
the policy applies.

| `SlowResponse` | What the bot does after the delay |
|---|---|
| `Warn(delay)` (default: 2.5 s) | Logs a warning. |
| `Defer(delay)` | Defers: a message defer for a command, an update defer for a component or a modal. |
| `Ignore` | Nothing. |

```rust
{{#include ../../../crates/cordial/tests/guide/responses.rs:slow}}
```

The automatic defer is not ephemeral. An ephemeral reply after it replaces the loading
message with an ephemeral follow-up message. Call `defer_ephemeral` yourself to keep the
loading message private.

The timer starts only for an interaction that has a handler. Autocomplete interactions
have no timer.

## Failed handlers

When a handler ends without a response, the bot logs a warning.

When a handler gives an error or panics, the bot logs the error (or gives it to the
`on_error` hook). Then it sends a default response that fits the response state:

| State | Default response |
|---|---|
| `Pending` (autocomplete) | Empty choices |
| `Pending` | An ephemeral "An error occurred." message |
| `Deferred` | An edit of the loading message to "An error occurred." |
| `DeferredUpdate` | An ephemeral follow-up message "An error occurred." |
| `Updated`, `Responded` | Nothing |

Set `BotBuilder::on_interaction_error` to answer yourself. Then the bot sends no default
response, and the error does not go to `on_error`. See
[Events and state](events.md#the-error-hooks).

## Ephemeral messages

An ephemeral message is visible only to the user of the interaction. It disappears when
the client restarts. Use it for errors, confirmations, and private data.

```rust
ctx.reply("Only you can see this.").ephemeral().await?;
```

## Components V2 messages

A Components V2 message has only components: text displays, containers, sections, media
galleries, separators, files, and action rows. It has no content and no embeds. Set the
flag with `InteractionMessage::components_v2`.

```rust
{{#include ../../../crates/cordial/tests/guide/responses.rs:v2}}
```

A Components V2 message cannot become a normal message again. An edit cannot remove the
flag.
