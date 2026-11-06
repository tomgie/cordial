//! Checks of the Discord limits at compile time.

use proc_macro2::Span;

/// The largest integer that Discord accepts in an integer option: 2^53 - 1.
pub(crate) const MAX_SAFE_INTEGER: i64 = (1 << 53) - 1;
/// The maximum number of options, subcommands, or groups of one level.
pub(crate) const OPTION_LIMIT: usize = 25;
/// The maximum number of choices of an option.
pub(crate) const CHOICE_LIMIT: usize = 25;
/// The maximum number of top-level components of a modal.
pub(crate) const MODAL_COMPONENT_LIMIT: usize = 5;

/// Checks the number of characters of a text.
pub(crate) fn length(
    text: &str,
    what: &str,
    min: usize,
    max: usize,
    span: Span,
) -> syn::Result<()> {
    let count = text.chars().count();
    if count < min || count > max {
        let message = if min == 0 {
            format!("the {what} has {count} characters; the maximum is {max}")
        } else {
            format!("the {what} has {count} characters; it must have {min} to {max}")
        };
        return Err(syn::Error::new(span, message));
    }
    Ok(())
}

/// Checks the name of a chat input command, a subcommand, a group, or an option.
///
/// Discord uses `^[-_'\p{L}\p{N}\p{sc=Deva}\p{sc=Thai}]{1,32}$` and wants lowercase letters
/// where a lowercase form exists. This check is an approximation of that rule.
pub(crate) fn chat_input_name(name: &str, span: Span) -> syn::Result<()> {
    length(name, "name", 1, 32, span)?;

    if name.chars().any(char::is_uppercase) {
        return Err(syn::Error::new(
            span,
            format!("the name `{name}` has uppercase letters; use lowercase letters"),
        ));
    }

    let is_allowed =
        |character: char| character.is_alphanumeric() || matches!(character, '-' | '_' | '\'');
    if let Some(bad) = name.chars().find(|&character| !is_allowed(character)) {
        return Err(syn::Error::new(
            span,
            format!(
                "the name `{name}` has the character `{bad}`; use only letters, digits, `-`, `_`, and `'`"
            ),
        ));
    }
    Ok(())
}

/// Checks that the limits of an option are not in the wrong order.
pub(crate) fn order<T: PartialOrd + Copy>(
    min: T,
    max: T,
    what: &str,
    span: Span,
) -> syn::Result<()> {
    if min > max {
        return Err(syn::Error::new(
            span,
            format!("the minimum {what} is more than the maximum {what}"),
        ));
    }
    Ok(())
}

/// Checks the number of items in a list.
pub(crate) fn count(count: usize, what: &str, max: usize, span: Span) -> syn::Result<()> {
    if count > max {
        return Err(syn::Error::new(
            span,
            format!("there are {count} {what}; Discord accepts a maximum of {max}"),
        ));
    }
    Ok(())
}
