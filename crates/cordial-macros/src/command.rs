//! `#[derive(Command)]` and the `#[command]` attribute.

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::ext::IdentExt;
use syn::meta::ParseNestedMeta;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Field, Fields, GenericArgument, Ident, LitStr, PathArguments, Type};

use crate::attr::{
    Errors, Number, once, parse_attrs, parse_crate_path, parse_flag, parse_length, parse_number,
    parse_str, unknown_key,
};
use crate::case::pascal_case;
use crate::check;
use crate::path::Paths;

/// The type of a top-level command.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommandKind {
    ChatInput,
    User,
    Message,
}

/// The keys of `#[command(...)]`.
#[derive(Default)]
pub(crate) struct CommandAttrs {
    pub(crate) name: Option<LitStr>,
    pub(crate) description: Option<LitStr>,
    pub(crate) kind: Option<(CommandKind, Span)>,
    pub(crate) permissions: Option<LitStr>,
    pub(crate) nsfw: Option<bool>,
    pub(crate) contexts: Option<LitStr>,
    pub(crate) integration_types: Option<LitStr>,
    pub(crate) framework: Option<syn::Path>,
}

const COMMAND_KEYS: &[&str] = &[
    "name",
    "description",
    "kind",
    "default_member_permissions",
    "nsfw",
    "contexts",
    "integration_types",
    "crate",
];

impl CommandAttrs {
    pub(crate) fn parse(attrs: &[syn::Attribute], errors: &mut Errors) -> Self {
        let mut this = Self::default();
        parse_attrs(attrs, "command", errors, |meta, key| match key {
            "name" => once(&mut this.name, meta, parse_str),
            "description" => once(&mut this.description, meta, parse_str),
            "kind" => once(&mut this.kind, meta, parse_kind),
            "default_member_permissions" => once(&mut this.permissions, meta, parse_str),
            "nsfw" => once(&mut this.nsfw, meta, parse_flag),
            "contexts" => once(&mut this.contexts, meta, parse_str),
            "integration_types" => once(&mut this.integration_types, meta, parse_str),
            "crate" => once(&mut this.framework, meta, parse_crate_path),
            _ => Err(unknown_key(meta, "command", COMMAND_KEYS)),
        });
        this
    }

    pub(crate) fn kind(&self) -> CommandKind {
        self.kind.map_or(CommandKind::ChatInput, |(kind, _)| kind)
    }

    /// Gives an error for each key that needs `name`, when `name` is absent.
    pub(crate) fn check_without_name(&self, errors: &mut Errors) {
        let spans = [
            self.description.as_ref().map(LitStr::span),
            self.kind.map(|(_, span)| span),
            self.permissions.as_ref().map(LitStr::span),
            self.contexts.as_ref().map(LitStr::span),
            self.integration_types.as_ref().map(LitStr::span),
        ];
        for span in spans.into_iter().flatten() {
            errors.push(syn::Error::new(
                span,
                "this key is for a top-level command; add `name = \"...\"` to `#[command]`",
            ));
        }
        if self.nsfw.is_some() {
            errors.push(syn::Error::new(
                Span::call_site(),
                "`nsfw` is for a top-level command; add `name = \"...\"` to `#[command]`",
            ));
        }
    }

    /// Checks the name and the description of a chat input command.
    pub(crate) fn check_chat_input(&self, name: &LitStr, errors: &mut Errors) {
        errors.check(check::chat_input_name(&name.value(), name.span()));

        match &self.description {
            Some(description) => errors.check(check::length(
                &description.value(),
                "description",
                1,
                100,
                description.span(),
            )),
            None => errors.push(syn::Error::new(
                name.span(),
                "a chat input command needs `description = \"...\"`",
            )),
        }
    }

    /// Gives the method calls that add the settings to a `CommandDefinition`.
    pub(crate) fn settings(&self, paths: &Paths, errors: &mut Errors) -> Vec<TokenStream> {
        let model = &paths.model;
        let mut settings = Vec::new();

        if let Some(literal) = &self.permissions {
            match permission_flags(literal, model) {
                Ok(flags) => settings.push(quote!(.default_member_permissions(#flags))),
                Err(error) => errors.push(error),
            }
        }

        if self.nsfw == Some(true) {
            settings.push(quote!(.nsfw(true)));
        }

        if let Some(literal) = &self.contexts {
            let enum_path = quote!(#model::interaction::InteractionContextType);
            match variant_list(literal, &enum_path, "context") {
                Ok(variants) => settings.push(quote!(.contexts([#(#variants),*]))),
                Err(error) => errors.push(error),
            }
        }

        if let Some(literal) = &self.integration_types {
            let enum_path = quote!(#model::application::ApplicationIntegrationType);
            match variant_list(literal, &enum_path, "integration type") {
                Ok(variants) => settings.push(quote!(.integration_types([#(#variants),*]))),
                Err(error) => errors.push(error),
            }
        }

        settings
    }
}

fn parse_kind(meta: &ParseNestedMeta<'_>) -> syn::Result<(CommandKind, Span)> {
    let literal = parse_str(meta)?;
    let kind = match literal.value().as_str() {
        "chat_input" => CommandKind::ChatInput,
        "user" => CommandKind::User,
        "message" => CommandKind::Message,
        _ => {
            return Err(syn::Error::new(
                literal.span(),
                "the kind must be \"chat_input\", \"user\", or \"message\"",
            ));
        }
    };
    Ok((kind, literal.span()))
}

/// Converts `"BAN_MEMBERS | KICK_MEMBERS"` to permission constants.
///
/// Each constant gets the span of the string, so a wrong name gives an error there.
fn permission_flags(literal: &LitStr, model: &TokenStream) -> syn::Result<TokenStream> {
    let text = literal.value();
    if text.trim().is_empty() {
        return Ok(quote!(#model::Permissions::empty()));
    }

    let mut flags = Vec::new();
    for part in text.split('|').map(str::trim) {
        let is_constant = part.starts_with(|c: char| c.is_ascii_uppercase())
            && part
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
        if !is_constant {
            return Err(syn::Error::new(
                literal.span(),
                format!(
                    "`{part}` is not a permission name; write the names like \"BAN_MEMBERS | KICK_MEMBERS\""
                ),
            ));
        }
        let ident = Ident::new(part, literal.span());
        flags.push(quote_spanned!(literal.span()=> #model::Permissions::#ident));
    }
    Ok(quote!(#(#flags)|*))
}

/// Converts `"guild, bot_dm"` to the variants `Guild` and `BotDm` of an enum.
fn variant_list(
    literal: &LitStr,
    enum_path: &TokenStream,
    what: &str,
) -> syn::Result<Vec<TokenStream>> {
    let text = literal.value();
    let mut variants = Vec::new();

    for part in text.split(',').map(str::trim) {
        let is_snake_case = part.starts_with(|c: char| c.is_ascii_lowercase())
            && part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !is_snake_case {
            return Err(syn::Error::new(
                literal.span(),
                format!(
                    "`{part}` is not a valid {what}; write the names in snake case, like \"guild, bot_dm\""
                ),
            ));
        }
        let ident = Ident::new(&pascal_case(part), literal.span());
        variants.push(quote_spanned!(literal.span()=> #enum_path::#ident));
    }
    Ok(variants)
}

/// Gives `T` for `Option<T>`. The check is on the syntax only.
pub(crate) fn option_inner(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }

    let segment = path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    match arguments.args.iter().collect::<Vec<_>>().as_slice() {
        [GenericArgument::Type(inner)] => Some(inner),
        _ => None,
    }
}

/// The keys of `#[option(...)]`.
#[derive(Default)]
struct OptionAttrs {
    name: Option<LitStr>,
    description: Option<LitStr>,
    min_value: Option<(Number, Span)>,
    max_value: Option<(Number, Span)>,
    min_length: Option<(u16, Span)>,
    max_length: Option<(u16, Span)>,
    autocomplete: Option<bool>,
    channel_types: Option<Vec<Ident>>,
}

const OPTION_KEYS: &[&str] = &[
    "name",
    "description",
    "min_value",
    "max_value",
    "min_length",
    "max_length",
    "autocomplete",
    "channel_types",
];

fn parse_channel_types(meta: &ParseNestedMeta<'_>) -> syn::Result<Vec<Ident>> {
    let mut types = Vec::new();
    meta.parse_nested_meta(|inner| {
        let ident = inner
            .path
            .get_ident()
            .cloned()
            .ok_or_else(|| inner.error("expected a `ChannelType` variant, like `GuildText`"))?;
        types.push(ident);
        Ok(())
    })?;
    Ok(types)
}

/// One field of a struct of options.
struct OptionField {
    member: Ident,
    name: String,
    span: Span,
    ty: Type,
    required: bool,
    description: String,
    settings: Vec<TokenStream>,
}

fn option_field(field: &Field, paths: &Paths, errors: &mut Errors) -> OptionField {
    let mut attrs = OptionAttrs::default();
    parse_attrs(&field.attrs, "option", errors, |meta, key| match key {
        "name" => once(&mut attrs.name, meta, parse_str),
        "description" => once(&mut attrs.description, meta, parse_str),
        "min_value" => once(&mut attrs.min_value, meta, parse_number),
        "max_value" => once(&mut attrs.max_value, meta, parse_number),
        "min_length" => once(&mut attrs.min_length, meta, parse_length),
        "max_length" => once(&mut attrs.max_length, meta, parse_length),
        "autocomplete" => once(&mut attrs.autocomplete, meta, parse_flag),
        "channel_types" => once(&mut attrs.channel_types, meta, parse_channel_types),
        _ => Err(unknown_key(meta, "option", OPTION_KEYS)),
    });

    // Callers give only named fields.
    let member = field
        .ident
        .clone()
        .unwrap_or_else(|| Ident::new("_", field.span()));
    let span = member.span();

    let (name, name_span) = match &attrs.name {
        Some(literal) => (literal.value(), literal.span()),
        None => (member.unraw().to_string(), member.span()),
    };
    errors.check(check::chat_input_name(&name, name_span));

    let description = match &attrs.description {
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
                "an option needs `#[option(description = \"...\")]`",
            ));
            String::new()
        }
    };

    let (ty, required) = match option_inner(&field.ty) {
        Some(inner) => (inner.clone(), false),
        None => (field.ty.clone(), true),
    };

    let settings = option_settings(&attrs, paths, errors);

    OptionField {
        member,
        name,
        span,
        ty,
        required,
        description,
        settings,
    }
}

/// Gives the statements that apply the attribute limits to `option`.
fn option_settings(attrs: &OptionAttrs, paths: &Paths, errors: &mut Errors) -> Vec<TokenStream> {
    let model = &paths.model;
    let mut settings = Vec::new();

    for (limit, field) in [
        (attrs.min_value, "min_value"),
        (attrs.max_value, "max_value"),
    ] {
        let Some((number, span)) = limit else {
            continue;
        };
        let field = Ident::new(field, span);
        let value = match number {
            Number::Integer(integer) => {
                if integer.abs() > check::MAX_SAFE_INTEGER {
                    errors.push(syn::Error::new(
                        span,
                        "Discord accepts integers from -(2^53 - 1) to 2^53 - 1",
                    ));
                }
                quote!(#model::command::NumberLimit::Integer(#integer))
            }
            Number::Float(float) => quote!(#model::command::NumberLimit::Number(#float)),
        };
        settings.push(quote!(option.#field = ::core::option::Option::Some(#value);));
    }
    if let (Some((min, _)), Some((max, span))) = (attrs.min_value, attrs.max_value) {
        errors.check(check::order(min.as_f64(), max.as_f64(), "value", span));
    }

    for (limit, field, min) in [
        (attrs.min_length, "min_length", 0),
        (attrs.max_length, "max_length", 1),
    ] {
        let Some((length, span)) = limit else {
            continue;
        };
        if !(min..=6000).contains(&length) {
            errors.push(syn::Error::new(
                span,
                format!("the {} must be from {min} to 6000", field.replace('_', " ")),
            ));
        }
        let field = Ident::new(field, span);
        settings.push(quote!(option.#field = ::core::option::Option::Some(#length);));
    }
    if let (Some((min, _)), Some((max, span))) = (attrs.min_length, attrs.max_length) {
        errors.check(check::order(min, max, "length", span));
    }

    if attrs.autocomplete == Some(true) {
        settings.push(quote!(option.autocomplete = true;));
    }

    if let Some(types) = &attrs.channel_types {
        let channel_type = quote!(#model::channel::ChannelType);
        let types = types
            .iter()
            .map(|ident| quote_spanned!(ident.span()=> #channel_type::#ident));
        settings.push(
            quote!(option.channel_types = ::core::option::Option::Some(::std::vec![#(#types),*]);),
        );
    }

    settings
}

/// Checks the rules of Discord for a list of options.
fn check_options(fields: &[OptionField], errors: &mut Errors) {
    if let Some(extra) = fields.get(check::OPTION_LIMIT) {
        errors.check(check::count(
            fields.len(),
            "options",
            check::OPTION_LIMIT,
            extra.span,
        ));
    }

    let mut has_optional = false;
    for field in fields {
        if field.required && has_optional {
            errors.push(syn::Error::new(
                field.span,
                "a required option must come before all optional options; move this field up or make it an `Option`",
            ));
        }
        has_optional |= !field.required;
    }

    for (index, field) in fields.iter().enumerate() {
        if fields[..index].iter().any(|other| other.name == field.name) {
            errors.push(syn::Error::new(
                field.span,
                format!("two options have the name `{}`", field.name),
            ));
        }
    }
}

/// Gives the `CommandOption` expression of one field.
fn option_definition(field: &OptionField, paths: &Paths) -> TokenStream {
    let framework = &paths.framework;
    let model = &paths.model;
    let OptionField {
        name,
        ty,
        required,
        description,
        settings,
        ..
    } = field;

    let arg = quote!(<#ty as #framework::CommandArg>);
    let option = quote! {
        #arg::configure(
            #model::command::CommandOption::new(#arg::KIND, #name, #description).required(#required)
        )
    };
    if settings.is_empty() {
        return option;
    }
    quote!({
        let mut option = #option;
        #(#settings)*
        option
    })
}

/// Gives the expression that parses one field from `input`.
fn option_parse(field: &OptionField) -> TokenStream {
    let OptionField {
        member,
        name,
        ty,
        required,
        ..
    } = field;
    if *required {
        quote!(#member: input.required::<#ty>(#name)?)
    } else {
        quote!(#member: input.optional::<#ty>(#name)?)
    }
}

pub(crate) fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(
            input.ident.span(),
            "`Command` needs a struct; use `CommandGroup` for an enum of subcommands",
        ));
    };

    let mut errors = Errors::default();
    let attrs = CommandAttrs::parse(&input.attrs, &mut errors);
    let paths = Paths::new(attrs.framework.clone());

    let output = match attrs.kind() {
        CommandKind::ChatInput => chat_input(input, &data.fields, &attrs, &paths, &mut errors),
        kind => context_menu(input, &data.fields, &attrs, kind, &paths, &mut errors),
    };

    errors.finish()?;
    Ok(output)
}

fn chat_input(
    input: &DeriveInput,
    fields: &Fields,
    attrs: &CommandAttrs,
    paths: &Paths,
    errors: &mut Errors,
) -> TokenStream {
    if let Fields::Unnamed(unnamed) = fields {
        errors.push(syn::Error::new(
            unnamed.span(),
            "a chat input command needs named fields; each field is one option",
        ));
    }

    let fields: Vec<OptionField> = fields
        .iter()
        .filter(|field| field.ident.is_some())
        .map(|field| option_field(field, paths, errors))
        .collect();
    check_options(&fields, errors);

    let mut output = options_impl(input, &fields, paths);

    match &attrs.name {
        Some(name) => {
            attrs.check_chat_input(name, errors);
            output.extend(chat_input_command_impl(input, attrs, name, paths, errors));
        }
        None => attrs.check_without_name(errors),
    }

    output
}

fn options_impl(input: &DeriveInput, fields: &[OptionField], paths: &Paths) -> TokenStream {
    let framework = &paths.framework;
    let model = &paths.model;
    let ident = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

    let definitions = fields.iter().map(|field| option_definition(field, paths));
    let parsers = fields.iter().map(option_parse);
    let body = if fields.is_empty() {
        quote! {
            let _ = input;
            ::core::result::Result::Ok(Self {})
        }
    } else {
        quote!(::core::result::Result::Ok(Self { #(#parsers,)* }))
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
                #body
            }
        }
    }
}

/// Implements `Command` for a chat input command. `CommandOptions` gives the options.
pub(crate) fn chat_input_command_impl(
    input: &DeriveInput,
    attrs: &CommandAttrs,
    name: &LitStr,
    paths: &Paths,
    errors: &mut Errors,
) -> TokenStream {
    let framework = &paths.framework;
    let model = &paths.model;
    let ident = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let description = attrs
        .description
        .as_ref()
        .map(LitStr::value)
        .unwrap_or_default();
    let settings = attrs.settings(paths, errors);

    quote! {
        #[automatically_derived]
        impl #impl_generics #framework::Command for #ident #type_generics #where_clause {
            const NAME: &'static str = #name;
            const KIND: #model::command::CommandType = #model::command::CommandType::ChatInput;

            fn definition() -> #model::command::CommandDefinition {
                let mut definition =
                    #model::command::CommandDefinition::chat_input(#name, #description) #(#settings)*;
                definition.options = <Self as #framework::CommandOptions>::options();
                definition
            }

            fn parse(
                data: &#model::interaction::CommandData,
            ) -> ::core::result::Result<Self, #framework::ParseError> {
                <Self as #framework::CommandOptions>::parse(
                    #framework::command::OptionsInput::from_data(data),
                )
            }
        }
    }
}

/// Implements `Command` for a user command or a message command.
///
/// Such a command has no options, so the struct does not get `CommandOptions`.
fn context_menu(
    input: &DeriveInput,
    fields: &Fields,
    attrs: &CommandAttrs,
    kind: CommandKind,
    paths: &Paths,
    errors: &mut Errors,
) -> TokenStream {
    let framework = &paths.framework;
    let model = &paths.model;
    let ident = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

    let Some(name) = &attrs.name else {
        errors.push(syn::Error::new(
            ident.span(),
            "a user or message command needs `#[command(name = \"...\")]`",
        ));
        return TokenStream::new();
    };
    errors.check(check::length(&name.value(), "name", 1, 32, name.span()));

    if let Some(description) = &attrs.description {
        errors.push(syn::Error::new(
            description.span(),
            "a user or message command has no description",
        ));
    }

    for field in fields {
        if let Some(attr) = field
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("option"))
        {
            errors.push(syn::Error::new(
                attr.span(),
                "a user or message command has no options",
            ));
        }
    }

    let [field] = fields.iter().collect::<Vec<_>>()[..] else {
        errors.push(syn::Error::new(
            ident.span(),
            "a user or message command needs exactly one field: the target, for example `ResolvedUser` or `Message`",
        ));
        return TokenStream::new();
    };

    let ty = &field.ty;
    let target = quote_spanned!(ty.span()=> <#ty as #framework::command::CommandTarget>);
    let value = quote!(#target::from_data(data)?);
    let construct = match &field.ident {
        Some(member) => quote!(Self { #member: #value }),
        None => quote!(Self(#value)),
    };

    let (variant, constructor, message) = match kind {
        CommandKind::User => (
            quote!(User),
            quote!(user),
            "the field of a user command must be a user target, for example `ResolvedUser`",
        ),
        _ => (
            quote!(Message),
            quote!(message),
            "the field of a message command must be a message target, for example `Message`",
        ),
    };
    let command_type = quote!(#model::command::CommandType::#variant);
    let settings = attrs.settings(paths, errors);

    // The check needs a concrete type, so a generic struct does not get it.
    let kind_check = if input.generics.params.is_empty() {
        quote_spanned! {ty.span()=>
            const _: () = ::core::assert!(
                ::core::matches!(#target::KIND, #command_type),
                #message,
            );
        }
    } else {
        TokenStream::new()
    };

    quote! {
        #kind_check

        #[automatically_derived]
        impl #impl_generics #framework::Command for #ident #type_generics #where_clause {
            const NAME: &'static str = #name;
            const KIND: #model::command::CommandType = #command_type;

            fn definition() -> #model::command::CommandDefinition {
                #model::command::CommandDefinition::#constructor(#name) #(#settings)*
            }

            fn parse(
                data: &#model::interaction::CommandData,
            ) -> ::core::result::Result<Self, #framework::ParseError> {
                ::core::result::Result::Ok(#construct)
            }
        }
    }
}
