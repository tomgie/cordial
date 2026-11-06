# Components and modals

Components are buttons, select menus, and other parts of a message. A modal is a form in
a popup window. When a user uses a component or submits a modal, Discord sends an
interaction with the `custom_id` of the component or the modal.

## Custom ID patterns

The bot routes component and modal interactions by `custom_id` patterns. A pattern has
literal text and named parameters in braces:

```text
vote:{poll}:{choice}
```

The `custom_id` carries the state. Thus a button works also after a restart of the bot.

| Rule | Example |
|---|---|
| A parameter is never empty. | `vote::yes` does not match. |
| A parameter ends where the next literal text matches first. | `vote:12:yes:no` gives `poll = 12`, `choice = yes:no`. |
| Two parameters need literal text between them. | `{a}{b}` is not valid. |
| A parameter name occurs once. | `{a}:{a}` is not valid. |
| The first pattern that matches wins. | Add specific patterns first. |

A `custom_id` has at most 100 characters. `build` gives an error for a pattern that is
not valid.

## Component handlers

Register a handler with `BotBuilder::component`. The handler gets a `ComponentContext`.

| Method | Gives |
|---|---|
| `param::<T>(name)` | The parameter, parsed into `T` with `FromStr`. |
| `params()` | All parameters. |
| `custom_id()` | The `custom_id`. |
| `values()` | The selected values of a select menu. |
| `into_inner()` | The `InteractionContext`. |

`ComponentContext` dereferences to `InteractionContext`, so all response methods work.
For a component, `update_message` edits the message that has the component.

The poll example makes the buttons:

```rust
{{#include ../../../crates/cordial/examples/poll.rs:buttons}}
```

It handles the votes:

```rust
{{#include ../../../crates/cordial/examples/poll.rs:vote}}
```

It registers the patterns:

```rust
{{#include ../../../crates/cordial/examples/poll.rs:builder}}
```

`Pattern::format` makes a `custom_id` from the same pattern, so the two cannot disagree:

```rust
{{#include ../../../crates/cordial/tests/guide/components.rs:format}}
```

## Modals

`#[derive(Modal)]` makes a modal from a struct. Each field is a text input in a label.

- A `String` field is required.
- An `Option<String>` field is not required. An empty text gives `None`.
- The struct attribute `#[modal(title = "...")]` gives the title (1 to 45 characters).

| Field key | Value |
|---|---|
| `label` | string, 1 to 45 characters (necessary) |
| `description` | string, up to 100 characters |
| `style` | `"short"` (default) or `"paragraph"` |
| `placeholder` | string, up to 100 characters |
| `min_length` | integer, 0 to 4000 |
| `max_length` | integer, 1 to 4000 |
| `custom_id` | string (default: the field name) |

```rust
{{#include ../../../crates/cordial/examples/modal_form.rs:modal}}
```

`Feedback::modal(custom_id)` makes an empty modal. `form.prefilled(custom_id)` makes a
modal with the values of `form`, for example to edit a value. Show the modal with
`ctx.modal`. Register the submit handler with `BotBuilder::modal`. The handler gets the
parsed struct.

```rust
{{#include ../../../crates/cordial/examples/modal_form.rs:handlers}}
```

```rust
{{#include ../../../crates/cordial/examples/modal_form.rs:builder}}
```

A modal is possible only as the initial response. A modal submit cannot open a second
modal.

## Collectors

A collector waits for an event in a handler. Use it for a short, live conversation, for
example "wait for the next press of this button". For state that must stay across
restarts, use the `custom_id` and the component router.

| Method | Waits for |
|---|---|
| `ctx.wait_for::<E>(filter, timeout)` | The next event of type `E` that matches the filter. |
| `ctx.collect::<E>(filter)` | All events of type `E` that match, until you drop the collector. |
| `ctx.wait_for_component(filter, timeout)` | The next component interaction that matches. |
| `ctx.wait_for_modal(filter, timeout)` | The next modal submit that matches. |

The methods give `None` at the timeout. An interaction that a collector takes does not go
to the router.

The poll example waits for the "Close" button of the author:

```rust
{{#include ../../../crates/cordial/examples/poll.rs:collector}}
```

The bot requests an event type from the gateway only when a handler or the cache needs
it. A collector does not add its type. When no handler uses the type of a collector,
request the type with `BotBuilder::extra_events`. The bot then also adds the intent. A
collector of a type that the bot does not request logs a warning.

```rust
{{#include ../../../crates/cordial/tests/guide/components.rs:wait_for}}
```

The message content in this example needs the `MESSAGE_CONTENT` intent. See
[Intents](intents.md).

A bot that gets interactions over HTTP gives them to the collectors too, so
`wait_for_component` and `wait_for_modal` work. Collectors of gateway events need the
gateway. See [HTTP interactions](http-interactions.md).
