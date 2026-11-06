# Start: a first bot

This chapter makes a bot with one slash command: `/ping`.

## Make the application

Do these steps in the [Developer Portal](https://discord.com/developers/applications):

1. Click "New Application". Type a name and click "Create".
2. Open the "Bot" page. Click "Reset Token" and copy the token.
3. Keep the token secret. A person with the token controls the bot.
4. Open the "Installation" page. Under "Guild Install", add the scopes `bot` and
   `applications.commands`.
5. Copy the install link. Open it in a browser and add the bot to a test server.

The bot needs no privileged intent for this chapter. The [Intents](intents.md) chapter
explains them.

## Give the token to the bot

The examples read the token from the environment variable `DISCORD_TOKEN`:

```sh
export DISCORD_TOKEN="the token from the Bot page"
```

On Windows PowerShell, use `$env:DISCORD_TOKEN = "..."`.

`Token::from_env` reads the variable. `Token::bot` makes a token from a string. Do not
write the token in the source code.

The `Debug` and `Display` output of a `Token` never shows the token. The memory of the
token is set to zero when the token is dropped.

## The code

This is `crates/cordial/examples/ping.rs`:

```rust
{{#include ../../../crates/cordial/examples/ping.rs}}
```

Run it from the repository root:

```sh
cargo run --example ping
```

Type `/ping` in the test server. The bot answers with the gateway latency.

## What the code does

1. `#[derive(Command)]` makes the command definition from the struct `Ping`. The struct
   has no fields, so the command has no options.
2. `ping` is the command handler. It gets a `CommandContext` and the parsed command.
3. `ctx.reply` sends the initial response to the interaction.
4. `ready` is an event handler. It runs at each `READY` event of a shard.
5. `Bot::builder` takes the token. `command` and `on` add the handlers.
6. `build` checks the settings. `run` connects and runs until Ctrl+C.

At the start, `run` does these steps:

1. It gets the application with `GET /applications/@me`.
2. It syncs the commands with Discord. Only the changes go to Discord.
3. It computes the intents from the handlers and checks them.
4. It starts the shards and gives each event to the handlers.

A global command can take some time to show in the Discord client. Restart the client
when the command does not show. For quick tests, register the command in one guild. See
[Command sync](commands.md#command-sync).

## The handler return type

A handler returns `()` or `Result<(), E>`, where `E` converts into `BoxError`. Thus `?`
works for all error types of the library.

When a handler gives an error, the bot logs it. It also tells the user, for example with
an ephemeral "An error occurred." message. Set an error hook to change this. See
[Events and state](events.md#the-error-hooks).

## Logs

The library writes logs with `tracing`. The example starts a subscriber at the level
`info`. Without a subscriber, you do not see the warnings of the library, for example the
intent warnings.
