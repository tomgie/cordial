//! `#[derive(Modal)]`.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::ext::IdentExt;
use syn::meta::ParseNestedMeta;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Field, Fields, Ident, LitStr, Type};

use crate::attr::{
    Errors, once, parse_attrs, parse_crate_path, parse_length, parse_str, unknown_key,
};
use crate::check;
use crate::command::option_inner;
use crate::path::Paths;

/// One text input.
struct Input {
    member: Ident,
    custom_id: String,
    span: Span,
    required: bool,
    label: String,
    settings: Vec<TokenStream>,
    label_settings: Vec<TokenStream>,
}

#[derive(Clone, Copy)]
enum Style {
    Short,
    Paragraph,
}

#[derive(Default)]
struct FieldAttrs {
    label: Option<LitStr>,
    description: Option<LitStr>,
    style: Option<Style>,
    placeholder: Option<LitStr>,
    min_length: Option<(u16, Span)>,
    max_length: Option<(u16, Span)>,
    custom_id: Option<LitStr>,
}

const MODAL_KEYS: &[&str] = &["title", "crate"];
const FIELD_KEYS: &[&str] = &[
    "label",
    "description",
    "style",
    "placeholder",
    "min_length",
    "max_length",
    "custom_id",
];

/// The maximum length of the text of a text input.
const TEXT_LIMIT: u16 = 4000;

fn parse_style(meta: &ParseNestedMeta<'_>) -> syn::Result<Style> {
    let literal = parse_str(meta)?;
    match literal.value().as_str() {
        "short" => Ok(Style::Short),
        "paragraph" => Ok(Style::Paragraph),
        _ => Err(syn::Error::new(
            literal.span(),
            "the style must be \"short\" or \"paragraph\"",
        )),
    }
}

/// Tells if the type is `String`. The check is on the syntax only.
fn is_string(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    path.qself.is_none()
        && path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "String" && segment.arguments.is_empty())
}

/// Checks an optional text attribute and gives the statement that sets it.
fn text_setting(
    literal: Option<&LitStr>,
    what: &str,
    max: usize,
    target: &TokenStream,
    errors: &mut Errors,
) -> Option<TokenStream> {
    let literal = literal?;
    errors.check(check::length(
        &literal.value(),
        what,
        1,
        max,
        literal.span(),
    ));
    Some(quote!(#target = ::core::option::Option::Some(::std::string::String::from(#literal));))
}

fn input(field: &Field, paths: &Paths, errors: &mut Errors) -> Input {
    let model = &paths.model;
    let mut attrs = FieldAttrs::default();
    parse_attrs(&field.attrs, "field", errors, |meta, key| match key {
        "label" => once(&mut attrs.label, meta, parse_str),
        "description" => once(&mut attrs.description, meta, parse_str),
        "style" => once(&mut attrs.style, meta, parse_style),
        "placeholder" => once(&mut attrs.placeholder, meta, parse_str),
        "min_length" => once(&mut attrs.min_length, meta, parse_length),
        "max_length" => once(&mut attrs.max_length, meta, parse_length),
        "custom_id" => once(&mut attrs.custom_id, meta, parse_str),
        _ => Err(unknown_key(meta, "field", FIELD_KEYS)),
    });

    // Callers give only named fields.
    let member = field
        .ident
        .clone()
        .unwrap_or_else(|| Ident::new("_", field.span()));
    let span = member.span();

    let required = if is_string(&field.ty) {
        true
    } else if option_inner(&field.ty).is_some_and(is_string) {
        false
    } else {
        errors.push(syn::Error::new(
            field.ty.span(),
            "a modal field must be `String` or `Option<String>`; only text inputs are supported for now",
        ));
        true
    };

    let custom_id = match &attrs.custom_id {
        Some(literal) => {
            errors.check(check::length(
                &literal.value(),
                "custom ID",
                1,
                100,
                literal.span(),
            ));
            literal.value()
        }
        None => member.unraw().to_string(),
    };

    let label = match &attrs.label {
        Some(literal) => {
            errors.check(check::length(
                &literal.value(),
                "label",
                1,
                45,
                literal.span(),
            ));
            literal.value()
        }
        None => {
            errors.push(syn::Error::new(
                span,
                "a modal field needs `#[field(label = \"...\")]`",
            ));
            String::new()
        }
    };

    let mut settings = Vec::new();
    if !required {
        settings.push(quote!(input.required = ::core::option::Option::Some(false);));
    }
    for (limit, name, min) in [
        (attrs.min_length, "min_length", 0),
        (attrs.max_length, "max_length", 1),
    ] {
        let Some((length, span)) = limit else {
            continue;
        };
        if !(min..=TEXT_LIMIT).contains(&length) {
            errors.push(syn::Error::new(
                span,
                format!(
                    "the {} must be from {min} to {TEXT_LIMIT}",
                    name.replace('_', " ")
                ),
            ));
        }
        let name = Ident::new(name, span);
        settings.push(quote!(input.#name = ::core::option::Option::Some(#length);));
    }
    if let (Some((min, _)), Some((max, span))) = (attrs.min_length, attrs.max_length) {
        errors.check(check::order(min, max, "length", span));
    }
    settings.extend(text_setting(
        attrs.placeholder.as_ref(),
        "placeholder",
        100,
        &quote!(input.placeholder),
        errors,
    ));

    let label_settings = text_setting(
        attrs.description.as_ref(),
        "description",
        100,
        &quote!(label.description),
        errors,
    )
    .into_iter()
    .collect();

    let style = match attrs.style.unwrap_or(Style::Short) {
        Style::Short => quote!(Short),
        Style::Paragraph => quote!(Paragraph),
    };
    let binding = if settings.is_empty() {
        quote!(let input)
    } else {
        quote!(let mut input)
    };
    settings.insert(
        0,
        quote!(#binding = #model::component::TextInput::new(#custom_id, #model::component::TextInputStyle::#style);),
    );

    Input {
        member,
        custom_id,
        span,
        required,
        label,
        settings,
        label_settings,
    }
}

fn check_inputs(inputs: &[Input], input: &DeriveInput, errors: &mut Errors) {
    if inputs.is_empty() {
        errors.push(syn::Error::new(
            input.ident.span(),
            "a modal needs at least one field",
        ));
    }

    if let Some(extra) = inputs.get(check::MODAL_COMPONENT_LIMIT) {
        errors.check(check::count(
            inputs.len(),
            "fields",
            check::MODAL_COMPONENT_LIMIT,
            extra.span,
        ));
    }

    for (index, field) in inputs.iter().enumerate() {
        if inputs[..index]
            .iter()
            .any(|other| other.custom_id == field.custom_id)
        {
            errors.push(syn::Error::new(
                field.span,
                format!("two fields have the custom ID `{}`", field.custom_id),
            ));
        }
    }
}

pub(crate) fn derive(input: &DeriveInput) -> syn::Result<TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(
            input.ident.span(),
            "`Modal` needs a struct with named fields",
        ));
    };

    let mut errors = Errors::default();
    let mut title: Option<LitStr> = None;
    let mut framework = None;
    parse_attrs(&input.attrs, "modal", &mut errors, |meta, key| match key {
        "title" => once(&mut title, meta, parse_str),
        "crate" => once(&mut framework, meta, parse_crate_path),
        _ => Err(unknown_key(meta, "modal", MODAL_KEYS)),
    });
    let paths = Paths::new(framework);

    match &title {
        Some(literal) => errors.check(check::length(
            &literal.value(),
            "title",
            1,
            45,
            literal.span(),
        )),
        None => errors.push(syn::Error::new(
            input.ident.span(),
            "a modal needs `#[modal(title = \"...\")]`",
        )),
    }

    if !matches!(data.fields, Fields::Named(_)) {
        errors.push(syn::Error::new(
            input.ident.span(),
            "`Modal` needs a struct with named fields",
        ));
    }
    let inputs: Vec<Input> = data
        .fields
        .iter()
        .filter(|field| field.ident.is_some())
        .map(|field| self::input(field, &paths, &mut errors))
        .collect();
    check_inputs(&inputs, input, &mut errors);

    errors.finish()?;
    let title = title.map(|literal| literal.value()).unwrap_or_default();
    Ok(generate(input, &inputs, &title, &paths))
}

fn generate(input: &DeriveInput, inputs: &[Input], title: &str, paths: &Paths) -> TokenStream {
    let framework = &paths.framework;
    let model = &paths.model;
    let ident = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let component = quote!(#model::component::Component);

    let components = inputs.iter().map(|field| {
        let Input {
            label,
            settings,
            label_settings,
            ..
        } = field;
        let label_binding = if label_settings.is_empty() {
            quote!(let label)
        } else {
            quote!(let mut label)
        };
        quote!({
            #(#settings)*
            #label_binding = #model::component::Label::new(#label, input);
            #(#label_settings)*
            #component::Label(label)
        })
    });

    let count = inputs.len();
    let values = inputs.iter().map(|field| {
        let member = &field.member;
        if field.required {
            quote!(::core::option::Option::Some(::core::clone::Clone::clone(&self.#member)))
        } else {
            quote!(::core::clone::Clone::clone(&self.#member))
        }
    });

    let parsers = inputs.iter().map(|field| {
        let Input {
            member, custom_id, ..
        } = field;
        let text = quote!(data.text(#custom_id));
        if field.required {
            quote! {
                #member: ::std::borrow::ToOwned::to_owned(#text.ok_or_else(|| {
                    #framework::ParseError::new(#framework::command::ParseErrorKind::MissingField)
                        .at(#custom_id)
                })?)
            }
        } else {
            quote! {
                #member: #text
                    .filter(|text| !text.is_empty())
                    .map(::std::borrow::ToOwned::to_owned)
            }
        }
    });

    quote! {
        #[automatically_derived]
        impl #impl_generics #framework::Modal for #ident #type_generics #where_clause {
            const TITLE: &'static str = #title;

            fn components() -> ::std::vec::Vec<#component> {
                ::std::vec![#(#components),*]
            }

            fn prefilled_components(&self) -> ::std::vec::Vec<#component> {
                let mut components = <Self as #framework::Modal>::components();
                let values: [::core::option::Option<::std::string::String>; #count] = [#(#values),*];
                for (component, value) in components.iter_mut().zip(values) {
                    // Discord can reject an empty value, so an empty text stays absent.
                    if let #component::Label(label) = component {
                        if let #component::TextInput(input) = &mut *label.component {
                            input.value = value.filter(|text| !text.is_empty());
                        }
                    }
                }
                components
            }

            fn parse(
                data: &#model::interaction::ModalSubmitData,
            ) -> ::core::result::Result<Self, #framework::ParseError> {
                ::core::result::Result::Ok(Self { #(#parsers,)* })
            }
        }
    }
}
