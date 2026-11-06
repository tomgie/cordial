//! Derive macros for the cordial framework.
//!
//! The macros generate command definitions and parsers from one source.
//! See `docs/ARCHITECTURE.md`, section 9.3.

use proc_macro::TokenStream;

/// Derives `CommandOptions`, and `Command` when the struct has `#[command(name = ...)]`.
#[proc_macro_derive(Command, attributes(command, option))]
pub fn derive_command(input: TokenStream) -> TokenStream {
    let _ = input;
    TokenStream::new()
}

/// Derives `CommandOptions` for an enum of subcommands, and `Command` when the enum has
/// `#[command(name = ...)]`.
#[proc_macro_derive(CommandGroup, attributes(command, subcommand, group))]
pub fn derive_command_group(input: TokenStream) -> TokenStream {
    let _ = input;
    TokenStream::new()
}

/// Derives `CommandArg` for an enum of fixed choices.
#[proc_macro_derive(Choice, attributes(choice))]
pub fn derive_choice(input: TokenStream) -> TokenStream {
    let _ = input;
    TokenStream::new()
}

/// Derives `Modal` for a struct of text fields.
#[proc_macro_derive(Modal, attributes(modal, field))]
pub fn derive_modal(input: TokenStream) -> TokenStream {
    let _ = input;
    TokenStream::new()
}
