//! `#[derive(CommandGroup)]`.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Fields, Ident, LitStr, Type, Variant};

use crate::attr::{Errors, once, parse_attrs, parse_str, unknown_key};
use crate::case::kebab_case;
use crate::check;
use crate::command::{CommandAttrs, CommandKind, chat_input_command_impl};
use crate::path::Paths;

/// One variant: a subcommand or a subcommand group.
struct Entry {
    ident: Ident,
    name: String,
    span: Span,
    description: String,
    is_group: bool,
    /// The type of the options. `None` for a unit subcommand.
    inner: Option<Type>,
}

const ENTRY_KEYS: &[&str] = &["name", "description"];

fn entry(variant: &Variant, errors: &mut Errors) -> Option<Entry> {
    let mut name: Option<LitStr> = None;
    let mut description: Option<LitStr> = None;
    let mut attribute_names = Vec::new();

    for kind in ["subcommand", "group"] {
        if !variant.attrs.iter().any(|attr| attr.path().is_ident(kind)) {
            continue;
        }
        attribute_names.push(kind);
        parse_attrs(&variant.attrs, kind, errors, |meta, key| match key {
            "name" => once(&mut name, meta, parse_str),
            "description" => once(&mut description, meta, parse_str),
            _ => Err(unknown_key(meta, kind, ENTRY_KEYS)),
        });
    }

    let span = variant.ident.span();
    let is_group = match attribute_names[..] {
        ["subcommand"] => false,
        ["group"] => true,
        [] => {
            errors.push(syn::Error::new(
                span,
                "add `#[subcommand(description = \"...\")]` or `#[group(description = \"...\")]` to this variant",
            ));
            return None;
        }
        _ => {
            errors.push(syn::Error::new(
                span,
                "a variant is a subcommand or a group, not both",
            ));
            return None;
        }
    };

    let (name, name_span) = match &name {
        Some(literal) => (literal.value(), literal.span()),
        None => (kebab_case(&variant.ident.to_string()), variant.ident.span()),
    };
    errors.check(check::chat_input_name(&name, name_span));

    let description = match &description {
        Some(literal) => {
            errors.check(check::length(
                &literal.value(),
                "description",
                1,
                100,
                literal.span(),
            ));
            literal.value()
        }
        None => {
            errors.push(syn::Error::new(
                span,
                "a subcommand or a group needs `description = \"...\"`",
            ));
            String::new()
        }
    };

    let inner = match &variant.fields {
        Fields::Unit if is_group => {
            errors.push(syn::Error::new(
                span,
                "a group needs one field: an enum that derives `CommandGroup`",
            ));
            None
        }
        Fields::Unit => None,
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Some(fields.unnamed[0].ty.clone()),
        _ => {
            errors.push(syn::Error::new(
                variant.fields.span(),
                "use one unnamed field, like `Variant(Options)`, or no field",
            ));
            None
        }
    };

    Some(Entry {
        ident: variant.ident.clone(),
        name,
        span,
        description,
        is_group,
        inner,
    })
}

fn check_entries(entries: &[Entry], input: &DeriveInput, errors: &mut Errors) {
    if entries.is_empty() {
        errors.push(syn::Error::new(
            input.ident.span(),
            "a command group needs at least one subcommand",
        ));
    }

    if let Some(extra) = entries.get(check::OPTION_LIMIT) {
        errors.check(check::count(
            entries.len(),
            "subcommands and groups",
            check::OPTION_LIMIT,
            extra.span,
        ));
    }

    for (index, entry) in entries.iter().enumerate() {
        if entries[..index]
            .iter()
            .any(|other| other.name == entry.name)
        {
            errors.push(syn::Error::new(
                entry.span,
                format!("two subcommands have the name `{}`", entry.name),
            ));
        }
    }
}

pub(crate) fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new(
            input.ident.span(),
            "`CommandGroup` needs an enum; use `Command` for a struct of options",
        ));
    };

    let mut errors = Errors::default();
    let attrs = CommandAttrs::parse(&input.attrs, &mut errors);
    let paths = Paths::new(attrs.framework.clone());

    let entries: Vec<Entry> = data
        .variants
        .iter()
        .filter_map(|variant| entry(variant, &mut errors))
        .collect();
    check_entries(&entries, input, &mut errors);

    let mut output = options_impl(input, &entries, &paths);

    if let Some((kind, span)) = attrs.kind
        && kind != CommandKind::ChatInput
    {
        errors.push(syn::Error::new(
            span,
            "a command with subcommands is always a chat input command",
        ));
    }
    match &attrs.name {
        Some(name) => {
            attrs.check_chat_input(name, &mut errors);
            output.extend(chat_input_command_impl(
                input,
                &attrs,
                name,
                &paths,
                &mut errors,
            ));
        }
        None => attrs.check_without_name(&mut errors),
    }

    errors.finish()?;
    Ok(output)
}

fn options_impl(input: &DeriveInput, entries: &[Entry], paths: &Paths) -> TokenStream {
    let framework = &paths.framework;
    let model = &paths.model;
    let ident = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

    let definitions = entries.iter().map(|entry| {
        let Entry {
            name, description, ..
        } = entry;
        let kind = if entry.is_group {
            quote!(#model::command::CommandOptionType::SubCommandGroup)
        } else {
            quote!(#model::command::CommandOptionType::SubCommand)
        };
        let option = quote!(#model::command::CommandOption::new(#kind, #name, #description));
        match &entry.inner {
            Some(inner) => quote!({
                let mut option = #option;
                option.options = <#inner as #framework::CommandOptions>::options();
                option
            }),
            None => option,
        }
    });

    let arms = entries.iter().map(|entry| {
        let Entry { ident, name, .. } = entry;
        match &entry.inner {
            Some(inner) => quote! {
                #name => <#inner as #framework::CommandOptions>::parse(options)
                    .map(Self::#ident)
                    .map_err(|error| error.at(name))
            },
            None => quote!(#name => ::core::result::Result::Ok(Self::#ident)),
        }
    });

    let options = if entries.iter().any(|entry| entry.inner.is_some()) {
        quote!(options)
    } else {
        quote!(_)
    };

    quote! {
        #[automatically_derived]
        impl #impl_generics #framework::CommandOptions for #ident #type_generics #where_clause {
            fn options() -> ::std::vec::Vec<#model::command::CommandOption> {
                ::std::vec![#(#definitions),*]
            }

            fn parse(
                input: #framework::command::OptionsInput<'_>,
            ) -> ::core::result::Result<Self, #framework::ParseError> {
                let (name, #options) = input.subcommand()?;
                match name {
                    #(#arms,)*
                    _ => ::core::result::Result::Err(#framework::ParseError::new(
                        #framework::command::ParseErrorKind::UnknownSubcommand(
                            ::std::borrow::ToOwned::to_owned(name),
                        ),
                    )),
                }
            }
        }
    }
}
