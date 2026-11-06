//! `#[derive(Choice)]`.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Fields, Ident, LitStr};

use crate::attr::{
    ChoiceValue, Errors, once, parse_attrs, parse_choice_value, parse_crate_path, parse_str,
    unknown_key,
};
use crate::case::snake_case;
use crate::check;
use crate::path::Paths;

/// The value type of all choices of an enum.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ValueKind {
    String,
    Integer,
    Float,
}

impl ValueKind {
    fn of(value: &ChoiceValue) -> Self {
        match value {
            ChoiceValue::String(_) => Self::String,
            ChoiceValue::Integer(_) => Self::Integer,
            ChoiceValue::Float(_) => Self::Float,
        }
    }
}

struct Entry {
    ident: Ident,
    name: String,
    value: Option<(ChoiceValue, Span)>,
    span: Span,
}

const ENUM_KEYS: &[&str] = &["crate"];
const VARIANT_KEYS: &[&str] = &["name", "value"];

pub(crate) fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new(
            input.ident.span(),
            "`Choice` needs an enum of unit variants",
        ));
    };

    let mut errors = Errors::default();
    let mut framework = None;
    parse_attrs(&input.attrs, "choice", &mut errors, |meta, key| match key {
        "crate" => once(&mut framework, meta, parse_crate_path),
        _ => Err(unknown_key(meta, "choice", ENUM_KEYS)),
    });
    let paths = Paths::new(framework);

    let mut entries = Vec::new();
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            errors.push(syn::Error::new(
                variant.fields.span(),
                "a choice is a unit variant; remove the fields",
            ));
        }

        let mut name: Option<LitStr> = None;
        let mut value = None;
        parse_attrs(
            &variant.attrs,
            "choice",
            &mut errors,
            |meta, key| match key {
                "name" => once(&mut name, meta, parse_str),
                "value" => once(&mut value, meta, parse_choice_value),
                _ => Err(unknown_key(meta, "choice", VARIANT_KEYS)),
            },
        );

        let name = match &name {
            Some(literal) => {
                errors.check(check::length(
                    &literal.value(),
                    "choice name",
                    1,
                    100,
                    literal.span(),
                ));
                literal.value()
            }
            None => variant.ident.to_string(),
        };
        entries.push(Entry {
            ident: variant.ident.clone(),
            name,
            value,
            span: variant.ident.span(),
        });
    }

    check_entries(&entries, input, &mut errors);
    let kind = value_kind(&entries, &mut errors);
    let values: Vec<ChoiceValue> = entries
        .iter()
        .map(|entry| match &entry.value {
            Some((value, _)) => value.clone(),
            None => ChoiceValue::String(snake_case(&entry.ident.to_string())),
        })
        .collect();
    check_values(&entries, &values, &mut errors);

    errors.finish()?;
    Ok(generate(input, &entries, &values, kind, &paths))
}

fn check_entries(entries: &[Entry], input: &DeriveInput, errors: &mut Errors) {
    if entries.is_empty() {
        errors.push(syn::Error::new(
            input.ident.span(),
            "a choice enum needs at least one variant",
        ));
    }

    if let Some(extra) = entries.get(check::CHOICE_LIMIT) {
        errors.check(check::count(
            entries.len(),
            "choices",
            check::CHOICE_LIMIT,
            extra.span,
        ));
    }
}

/// Finds the one value type of all choices. A variant without a value has a string value.
fn value_kind(entries: &[Entry], errors: &mut Errors) -> ValueKind {
    let first_number = entries.iter().find_map(|entry| {
        let (value, _) = entry.value.as_ref()?;
        let kind = ValueKind::of(value);
        (kind != ValueKind::String).then_some(kind)
    });
    let kind = first_number.unwrap_or(ValueKind::String);

    for entry in entries {
        match &entry.value {
            Some((value, span)) if ValueKind::of(value) != kind => {
                errors.push(syn::Error::new(
                    *span,
                    "all choices must have values of one type: strings, integers, or floats",
                ));
            }
            None if kind != ValueKind::String => {
                errors.push(syn::Error::new(
                    entry.span,
                    "give a number `value` to each choice, because one choice has a number value",
                ));
            }
            _ => {}
        }
    }
    kind
}

fn check_values(entries: &[Entry], values: &[ChoiceValue], errors: &mut Errors) {
    for (index, (entry, value)) in entries.iter().zip(values).enumerate() {
        let span = entry.value.as_ref().map_or(entry.span, |(_, span)| *span);
        match value {
            ChoiceValue::String(text) => {
                errors.check(check::length(text, "choice value", 1, 100, span));
            }
            ChoiceValue::Integer(integer) if integer.abs() > check::MAX_SAFE_INTEGER => {
                errors.push(syn::Error::new(
                    span,
                    "Discord accepts integers from -(2^53 - 1) to 2^53 - 1",
                ));
            }
            _ => {}
        }

        let is_duplicate = values[..index].iter().any(|other| match (other, value) {
            (ChoiceValue::String(a), ChoiceValue::String(b)) => a == b,
            (ChoiceValue::Integer(a), ChoiceValue::Integer(b)) => a == b,
            (ChoiceValue::Float(a), ChoiceValue::Float(b)) => a.to_bits() == b.to_bits(),
            _ => false,
        });
        if is_duplicate {
            errors.push(syn::Error::new(span, "two choices have the same value"));
        }
    }
}

fn value_tokens(value: &ChoiceValue) -> TokenStream {
    match value {
        ChoiceValue::String(text) => quote!(#text),
        ChoiceValue::Integer(integer) => quote!(#integer),
        ChoiceValue::Float(float) => quote!(#float),
    }
}

fn generate(
    input: &DeriveInput,
    entries: &[Entry],
    values: &[ChoiceValue],
    kind: ValueKind,
    paths: &Paths,
) -> TokenStream {
    let framework = &paths.framework;
    let model = &paths.model;
    let ident = &input.ident;
    let visibility = &input.vis;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

    let variants: Vec<&Ident> = entries.iter().map(|entry| &entry.ident).collect();
    let names: Vec<&String> = entries.iter().map(|entry| &entry.name).collect();
    let values: Vec<TokenStream> = values.iter().map(value_tokens).collect();

    let (option_type, value_type, read) = match kind {
        ValueKind::String => (quote!(String), quote!(&'static str), quote!(as_str)),
        ValueKind::Integer => (quote!(Integer), quote!(i64), quote!(as_i64)),
        ValueKind::Float => (quote!(Number), quote!(f64), quote!(as_f64)),
    };
    let unknown = match kind {
        ValueKind::String => quote!(::std::borrow::ToOwned::to_owned(value)),
        _ => quote!(::std::string::ToString::to_string(&value)),
    };

    // Float values cannot be match patterns, so they use comparisons.
    let lookup = if kind == ValueKind::Float {
        quote! {
            #(
                if value == #values {
                    return ::core::result::Result::Ok(Self::#variants);
                }
            )*
        }
    } else {
        quote! {
            match value {
                #(#values => return ::core::result::Result::Ok(Self::#variants),)*
                _ => {}
            }
        }
    };

    quote! {
        #[automatically_derived]
        impl #impl_generics #framework::CommandArg for #ident #type_generics #where_clause {
            const KIND: #model::command::CommandOptionType =
                #model::command::CommandOptionType::#option_type;

            fn configure(
                mut option: #model::command::CommandOption,
            ) -> #model::command::CommandOption {
                option.choices = ::core::option::Option::Some(::std::vec![
                    #(#model::command::CommandOptionChoice::new(#names, #values)),*
                ]);
                option
            }

            #[allow(clippy::float_cmp)]
            fn parse(
                value: &#model::interaction::CommandOptionValue,
                _: ::core::option::Option<&#model::interaction::Resolved>,
            ) -> ::core::result::Result<Self, #framework::ParseError> {
                let ::core::option::Option::Some(value) = value.#read() else {
                    return ::core::result::Result::Err(#framework::ParseError::new(
                        #framework::command::ParseErrorKind::WrongType {
                            expected: <Self as #framework::CommandArg>::KIND,
                        },
                    ));
                };
                #lookup
                ::core::result::Result::Err(#framework::ParseError::new(
                    #framework::command::ParseErrorKind::UnknownChoice(#unknown),
                ))
            }
        }

        #[automatically_derived]
        #[allow(dead_code)]
        impl #impl_generics #ident #type_generics #where_clause {
            #[doc = "Gives the name of the choice that Discord shows."]
            #[must_use]
            #visibility const fn choice_name(&self) -> &'static str {
                match self {
                    #(Self::#variants => #names,)*
                }
            }

            #[doc = "Gives the value of the choice that Discord sends."]
            #[must_use]
            #visibility const fn choice_value(&self) -> #value_type {
                match self {
                    #(Self::#variants => #values,)*
                }
            }
        }
    }
}
