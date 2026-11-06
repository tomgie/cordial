//! Derive macros for the cordial framework.
//!
//! The macros generate command definitions and parsers from one source.
//! See `docs/ARCHITECTURE.md`, section 9.3.
//!
//! The generated code finds the framework through a direct dependency on `cordial-framework`
//! first, and then through the facade crate `cordial`. Use `crate = "path"` in the type
//! attribute (`#[command]`, `#[choice]`, or `#[modal]`) to give a different path.

mod attr;
mod case;
mod check;
mod choice;
mod command;
mod group;
mod modal;
mod path;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// Derives `CommandOptions`, and `Command` when the struct has `#[command(name = ...)]`.
///
/// # Struct attribute `#[command(...)]`
///
/// | Key | Value | Use |
/// |---|---|---|
/// | `name` | string | Makes the struct a top-level command. |
/// | `description` | string, 1 to 100 characters | Needed for a chat input command. |
/// | `kind` | `"chat_input"` (default), `"user"`, `"message"` | The command type. |
/// | `default_member_permissions` | string, like `"BAN_MEMBERS \| KICK_MEMBERS"` | `""` means administrators only. |
/// | `nsfw` | flag | Age-restricted command. |
/// | `contexts` | string, like `"guild, bot_dm, private_channel"` | Interaction contexts. |
/// | `integration_types` | string, like `"guild_install, user_install"` | Installation contexts. |
/// | `crate` | string path | The path of the framework crate. |
///
/// # Field attribute `#[option(...)]` (chat input only)
///
/// | Key | Value |
/// |---|---|
/// | `description` | string, 1 to 100 characters (required) |
/// | `name` | string (default: the field name) |
/// | `min_value`, `max_value` | integer or float literal |
/// | `min_length`, `max_length` | integer, 0 to 6000 |
/// | `autocomplete` | flag |
/// | `channel_types(GuildText, ...)` | `ChannelType` variant names |
///
/// A field of type `Option<T>` is not required. Required fields must come first.
///
/// A user command or a message command has exactly one field. Its type implements
/// `CommandTarget`. Such a struct does not implement `CommandOptions`.
#[proc_macro_derive(Command, attributes(command, option))]
pub fn derive_command(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    command::derive(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives `CommandOptions` for an enum of subcommands, and `Command` when the enum has
/// `#[command(name = ...)]`.
///
/// Each variant has `#[subcommand(name = "...", description = "...")]` or
/// `#[group(name = "...", description = "...")]`. The default name is the variant name in
/// kebab case (`ShowAll` gives `show-all`).
///
/// - A subcommand variant has one field of a type that derives `Command` (the options), or no
///   field.
/// - A group variant has one field of an enum that derives `CommandGroup`.
///
/// Discord accepts only one level of groups: a group can contain only subcommands. The macro
/// cannot see the field type, so it does not check this. Discord rejects the definition.
///
/// The `#[command(...)]` attribute is the same as for `Command`, but `kind` must be
/// `"chat_input"`.
#[proc_macro_derive(CommandGroup, attributes(command, subcommand, group))]
pub fn derive_command_group(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    group::derive(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives `CommandArg` for an enum of fixed choices.
///
/// Each unit variant can have `#[choice(name = "...", value = ...)]`.
///
/// - The default name is the variant name.
/// - The default value is the variant name in snake case, as a string.
/// - A value can be a string, an integer, or a float literal. All values must have one type.
///   Integer values give an integer option, float values give a number option.
///
/// The enum gets the methods `choice_name` and `choice_value`. Use
/// `#[choice(crate = "path")]` on the enum to give the path of the framework crate.
#[proc_macro_derive(Choice, attributes(choice))]
pub fn derive_choice(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    choice::derive(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derives `Modal` for a struct of text fields.
///
/// The struct has `#[modal(title = "...")]`. The title has 1 to 45 characters.
///
/// Each field is a text input in a label, with `#[field(...)]`:
///
/// | Key | Value |
/// |---|---|
/// | `label` | string, 1 to 45 characters (required) |
/// | `description` | string, up to 100 characters |
/// | `style` | `"short"` (default) or `"paragraph"` |
/// | `placeholder` | string, up to 100 characters |
/// | `min_length` | integer, 0 to 4000 |
/// | `max_length` | integer, 1 to 4000 |
/// | `custom_id` | string (default: the field name) |
///
/// A `String` field is required. An `Option<String>` field is not required, and an empty
/// text gives `None`.
#[proc_macro_derive(Modal, attributes(modal, field))]
pub fn derive_modal(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    modal::derive(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
