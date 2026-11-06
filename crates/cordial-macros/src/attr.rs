//! Shared parts of attribute parsing.

use proc_macro2::Span;
use syn::meta::ParseNestedMeta;
use syn::{Attribute, Lit, LitBool, LitInt, LitStr, Token};

/// Collects errors, so that one run of the macro reports all of them.
#[derive(Default)]
pub(crate) struct Errors {
    error: Option<syn::Error>,
}

impl Errors {
    pub(crate) fn push(&mut self, error: syn::Error) {
        match &mut self.error {
            Some(first) => first.combine(error),
            None => self.error = Some(error),
        }
    }

    pub(crate) fn check(&mut self, result: syn::Result<()>) {
        if let Err(error) = result {
            self.push(error);
        }
    }

    pub(crate) fn finish(self) -> syn::Result<()> {
        self.error.map_or(Ok(()), Err)
    }
}

/// A number literal in an attribute.
#[derive(Clone, Copy)]
pub(crate) enum Number {
    Integer(i64),
    Float(f64),
}

impl Number {
    #[allow(clippy::cast_precision_loss)]
    pub(crate) fn as_f64(self) -> f64 {
        match self {
            Self::Integer(value) => value as f64,
            Self::Float(value) => value,
        }
    }
}

/// A literal value of a choice.
#[derive(Clone)]
pub(crate) enum ChoiceValue {
    String(String),
    Integer(i64),
    Float(f64),
}

/// Runs `handler` for each key of each attribute with the name `name`.
pub(crate) fn parse_attrs(
    attrs: &[Attribute],
    name: &str,
    errors: &mut Errors,
    mut handler: impl FnMut(&ParseNestedMeta<'_>, &str) -> syn::Result<()>,
) {
    for attr in attrs.iter().filter(|attr| attr.path().is_ident(name)) {
        let result = attr.parse_nested_meta(|meta| {
            let key = meta
                .path
                .get_ident()
                .map(ToString::to_string)
                .unwrap_or_default();
            handler(&meta, &key)
        });
        errors.check(result);
    }
}

/// Gives the error for a key that the attribute does not know.
pub(crate) fn unknown_key(meta: &ParseNestedMeta<'_>, attr: &str, keys: &[&str]) -> syn::Error {
    let key = meta
        .path
        .get_ident()
        .map(ToString::to_string)
        .unwrap_or_default();
    meta.error(format!(
        "unknown key `{key}` in `#[{attr}]`; the known keys are: {}",
        keys.join(", ")
    ))
}

/// Parses a value into `slot`, and gives an error when the key occurs again.
pub(crate) fn once<T>(
    slot: &mut Option<T>,
    meta: &ParseNestedMeta<'_>,
    parse: impl FnOnce(&ParseNestedMeta<'_>) -> syn::Result<T>,
) -> syn::Result<()> {
    if slot.is_some() {
        return Err(meta.error("this key occurs more than once"));
    }

    *slot = Some(parse(meta)?);
    Ok(())
}

pub(crate) fn parse_str(meta: &ParseNestedMeta<'_>) -> syn::Result<LitStr> {
    meta.value()?.parse()
}

/// Parses `key` or `key = true` or `key = false`.
pub(crate) fn parse_flag(meta: &ParseNestedMeta<'_>) -> syn::Result<bool> {
    if meta.input.peek(Token![=]) {
        let flag: LitBool = meta.value()?.parse()?;
        return Ok(flag.value);
    }
    Ok(true)
}

pub(crate) fn parse_length(meta: &ParseNestedMeta<'_>) -> syn::Result<(u16, Span)> {
    let literal: LitInt = meta.value()?.parse()?;
    let length = literal.base10_parse::<u16>()?;
    Ok((length, literal.span()))
}

/// Parses an integer or a float literal, with an optional minus sign.
pub(crate) fn parse_number(meta: &ParseNestedMeta<'_>) -> syn::Result<(Number, Span)> {
    let input = meta.value()?;
    let minus: Option<Token![-]> = input.parse()?;
    let literal: Lit = input.parse()?;
    let span = literal.span();
    let sign = if minus.is_some() { -1_i32 } else { 1_i32 };

    let number = match &literal {
        Lit::Int(integer) => Number::Integer(i64::from(sign) * integer.base10_parse::<i64>()?),
        Lit::Float(float) => Number::Float(f64::from(sign) * float.base10_parse::<f64>()?),
        _ => {
            return Err(syn::Error::new(
                span,
                "expected an integer or a float literal",
            ));
        }
    };
    if let Number::Float(float) = number {
        check_finite(float, span)?;
    }
    Ok((number, span))
}

/// Parses a string, an integer, or a float literal, with an optional minus sign for numbers.
pub(crate) fn parse_choice_value(meta: &ParseNestedMeta<'_>) -> syn::Result<(ChoiceValue, Span)> {
    let input = meta.value()?;
    let minus: Option<Token![-]> = input.parse()?;
    let literal: Lit = input.parse()?;
    let span = literal.span();
    let sign = if minus.is_some() { -1_i32 } else { 1_i32 };

    let value = match &literal {
        Lit::Str(text) if minus.is_none() => ChoiceValue::String(text.value()),
        Lit::Int(integer) => ChoiceValue::Integer(i64::from(sign) * integer.base10_parse::<i64>()?),
        Lit::Float(float) => ChoiceValue::Float(f64::from(sign) * float.base10_parse::<f64>()?),
        _ => {
            return Err(syn::Error::new(
                span,
                "expected a string, an integer, or a float literal",
            ));
        }
    };
    if let ChoiceValue::Float(float) = value {
        check_finite(float, span)?;
    }
    Ok((value, span))
}

/// A float that is too large becomes infinite. The literal tokens cannot hold it.
fn check_finite(float: f64, span: Span) -> syn::Result<()> {
    if float.is_finite() {
        Ok(())
    } else {
        Err(syn::Error::new(span, "the number is too large"))
    }
}

/// Parses `crate = "path"`.
pub(crate) fn parse_crate_path(meta: &ParseNestedMeta<'_>) -> syn::Result<syn::Path> {
    let literal = parse_str(meta)?;
    literal.parse()
}
