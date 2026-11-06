# Commands

An application command is a slash command, a user command, or a message command. In
cordial, one Rust type gives the definition for Discord and the parser for the interaction
data. Thus the two cannot disagree.

| Derive | Input | Use |
|---|---|---|
| `#[derive(Command)]` | a struct | A command with options, or a set of options |
| `#[derive(CommandGroup)]` | an enum | A command with subcommands, or a subcommand group |
| `#[derive(Choice)]` | an enum of unit variants | The type of an option with fixed choices |
| `#[derive(Modal)]` | a struct of text fields | A modal. See [Components and modals](components.md). |

The macros check the Discord limits when they compile. For example, a name with an
uppercase letter or a description with 101 characters gives a compile error.

## A command with options

Each named field is one option. The field attribute `#[option(...)]` gives the details.

```rust
{{#include ../../../crates/cordial/tests/guide/commands.rs:options}}
```

Register the handler with `BotBuilder::command`. The bot finds the command type from the
second argument of the handler.

### The command attribute

| Key | Value | Use |
|---|---|---|
| `name` | string, 1 to 32 characters | Makes the type a top-level command. |
| `description` | string, 1 to 100 characters | Necessary for a slash command. |
| `kind` | `"chat_input"` (default), `"user"`, `"message"` | The command type. |
| `default_member_permissions` | string, like `"BAN_MEMBERS \| KICK_MEMBERS"` | `""` means administrators only. |
| `nsfw` | flag | An age-restricted command. |
| `contexts` | string, like `"guild, bot_dm, private_channel"` | Where the command shows. |
| `integration_types` | string, like `"guild_install, user_install"` | How the app is installed. |
| `crate` | string path | The path of the framework crate, when the macro cannot find it. |

A slash command name has only lowercase letters, numbers, `-`, `_`, and `'`.

### The option attribute

| Key | Value |
|---|---|
| `description` | string, 1 to 100 characters (necessary) |
| `name` | string (default: the field name) |
| `min_value`, `max_value` | integer or float literal |
| `min_length`, `max_length` | integer, 0 to 6000 |
| `autocomplete` | flag |
| `channel_types(GuildText, ...)` | `ChannelType` variant names |

### Option types

| Rust type | Discord option type | Notes |
|---|---|---|
| `String` | String | |
| `bool` | Boolean | |
| `i64` | Integer | |
| `i8`, `i16`, `i32`, `u8`, `u16`, `u32`, `u64` | Integer | The type limits become `min_value` and `max_value`. |
| `f64` | Number | |
| `UserId` | User | The ID only. |
| `User` | User | The resolved user. |
| `ResolvedUser` | User | The user and the member, when the command ran in a guild. |
| `ChannelId` | Channel | The ID only. |
| `Channel` | Channel | A partial channel: ID, name, type, permissions, thread data. |
| `RoleId` | Role | The ID only. |
| `Role` | Role | The resolved role. |
| `GenericId` | Mentionable | The ID only. |
| `Mentionable` | Mentionable | A `ResolvedUser` or a `Role`. |
| `Attachment` | Attachment | The uploaded file. |
| an enum with `#[derive(Choice)]` | String, Integer, or Number | The option has fixed choices. |

Discord accepts integers from -(2^53 - 1) to 2^53 - 1. For `u64`, the maximum is
2^53 - 1.

### Optional options

A field of type `Option<T>` is not required. The user can leave it empty. Required fields
must come before optional fields, because Discord needs this order. The macro gives an
error for a required field after an optional field.

### Choices

`#[derive(Choice)]` makes an option type with fixed choices:

- The default name of a choice is the variant name.
- The default value is the variant name in snake case, as a string.
- A value can be a string, an integer, or a float. All values must have the same type.
- An integer value gives an Integer option. A float value gives a Number option.

```rust
{{#include ../../../crates/cordial/examples/moderation.rs:choice}}
```

The enum gets the methods `choice_name` and `choice_value`. Discord accepts up to 25
choices.

## Subcommands and groups

`#[derive(CommandGroup)]` makes a command from an enum. Each variant is a subcommand or a
subcommand group:

- `#[subcommand(description = "...")]`: a subcommand. The variant has no field, or one
  field of a type that derives `Command` (the options of the subcommand).
- `#[group(description = "...")]`: a group. The variant has one field of an enum that
  derives `CommandGroup`.

The default name is the variant name in kebab case (`ShowAll` gives `show-all`). Give
`name = "..."` to change it.

```rust
{{#include ../../../crates/cordial/tests/guide/commands.rs:group}}
```

Discord accepts only one level of groups. A group can contain only subcommands. The macro
cannot see the field type, so it does not check this. Discord rejects the definition at
the sync.

## User commands and message commands

A user command shows in the context menu of a user ("Apps"). A message command shows in
the context menu of a message. They have no description and no options. The struct has
exactly one unnamed field: the target.

| `kind` | Target type |
|---|---|
| `"user"` | `ResolvedUser` |
| `"message"` | `Message` |

```rust
{{#include ../../../crates/cordial/examples/user_app.rs:targets}}
```

The name of a user or message command can have uppercase letters and spaces.

## Permissions and contexts

`default_member_permissions` hides the command from members without the permissions.
Server administrators can change this setting in the server settings. Thus a check in the
bot is a good second guard for a dangerous command.

`contexts` sets where the command shows:

| Value | Where |
|---|---|
| `guild` | In servers |
| `bot_dm` | In the direct messages with the bot |
| `private_channel` | In other direct messages and group DMs (needs `user_install`) |

`integration_types` sets how a user or a server can install the app:

| Value | Installation |
|---|---|
| `guild_install` | Added to a server |
| `user_install` | Added to a user account |

A command for a user-installed app:

```rust
{{#include ../../../crates/cordial/examples/user_app.rs:roll}}
```

## Checks and cooldowns

A check runs before the handler. When a check fails, the bot answers with an ephemeral
message and does not run the handler. A cooldown sets a minimum time between two uses.

| Check | Allows the command |
|---|---|
| `Check::guild_only()` | Only in guilds. Prefer `contexts = "guild"`. |
| `Check::permissions(permissions)` | Only to members with all of the permissions in the channel. |
| `Check::users(user_ids)` | Only to these users. |
| `Check::new(async_fn)` | When your function gives `Ok(())`. |

| Cooldown scope | Counts separately for |
|---|---|
| `CooldownScope::User` | Each user |
| `CooldownScope::Guild` | Each guild (each user in DMs) |
| `CooldownScope::Channel` | Each channel |
| `CooldownScope::Global` | All uses together |

Add checks and a cooldown with `BotBuilder::command_with`:

```rust
{{#include ../../../crates/cordial/tests/guide/commands.rs:checks}}
```

The checks run in the order that you add them. The cooldown runs after the checks. A
panic in a check gets the same error handling as a panic in the handler. The moderation
example uses a permission check and a cooldown:

```rust
{{#include ../../../crates/cordial/examples/moderation.rs:builder}}
```

## Autocomplete

An option with `autocomplete` gets suggestions while the user types. The autocomplete
handler gets the focused option and gives up to 25 choices. The bot sends only the first
25 choices.

```rust
{{#include ../../../crates/cordial/tests/guide/commands.rs:autocomplete}}
```

`Focused` has the option name, the text that the user typed so far, and the path of the
subcommand. Numbers come as text, because the user can type a partial number.

An autocomplete handler must answer within 3 seconds. The bot does not defer it.

## Localization

A localizer gives the translations of names, descriptions, and choices. The bot adds them
to the definitions before the sync. A function `Fn(&LocalizationKey) -> Localizations` is
a localizer.

```rust
{{#include ../../../crates/cordial/tests/guide/commands.rs:localize}}
```

`LocalizationKey::dotted` gives keys like these:

| Text | Key |
|---|---|
| Command name | `ban.name` |
| Command description | `ban.description` |
| Option description | `ban.options.reason.description` |
| Choice name | `ban.options.kind.choices.Spam` |
| Option of a subcommand | `warn.options.add.options.user.description` |

Use the locale of the user to translate the replies: `ctx.locale()`.

## Command sync

At each start, the bot compares its commands with the commands on Discord. It sends only
the changes.

1. It gets the remote commands with their localizations.
2. It makes a plan: create, update, rename, delete, or unchanged, for each command.
3. It writes the summary of the plan to the log.
4. It sends the changes with the strategy of the options.

The sync compares the commands by the type and the name. It deletes a remote command that
has no local definition. The Entry Point command of an Activity stays, unless you set
`SyncOptions::delete_entry_point`.

| Strategy | Requests |
|---|---|
| `SyncStrategy::Granular` (default) | One request for each change. Unchanged commands stay untouched. |
| `SyncStrategy::Bulk` | One bulk overwrite with all commands, when the plan has changes. |
| `SyncStrategy::Auto { threshold }` | `Bulk` when the plan has more changes than the threshold. |

| `CommandSync` | When the bot syncs |
|---|---|
| `CommandSync::Default` (default) | At each start, with the default options. |
| `CommandSync::OnStart(options)` | At each start, with these options. |
| `CommandSync::Off` | Never. Use this when another tool registers the commands. |

```rust
{{#include ../../../crates/cordial/tests/guide/commands.rs:sync}}
```

- `previous_name` gives an old name of the command. The sync renames the command, so it
  keeps its ID and the permission settings of the servers.
- `guilds` registers the command only in some guilds. A guild command shows at once. Use
  it for tests.
- `dry_run(true)` makes the plan and writes it to the log, but sends no change.
- `sync_guilds` names guilds to sync also when they have no commands now.

The bot syncs the global scope and each guild scope that has commands. A guild that has
no commands in the code any more is not synced, so its old commands stay. Give such a
guild to `BotBuilder::sync_guilds`, for example after a command moves from a test guild
to global. The sync then removes the old commands of the guild.

`Bot::command_definitions` gives the definitions with the translations. Use it with the
`sync` function to register the commands from a separate program.
