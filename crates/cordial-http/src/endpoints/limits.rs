//! Checks of documented limits that more than one endpoint uses.

use cordial_model::validate::ValidationError;

/// Checks that a text has `min` to `max` characters.
pub(crate) fn length(
    field: &str,
    text: &str,
    min: usize,
    max: usize,
) -> Result<(), ValidationError> {
    let count = text.chars().count();
    if count < min || count > max {
        return Err(ValidationError::new(
            field,
            format!("has {count} characters; it must have {min} to {max}"),
        ));
    }
    Ok(())
}

/// Checks that an optional text has `min` to `max` characters.
pub(crate) fn optional_length(
    field: &str,
    text: Option<&str>,
    min: usize,
    max: usize,
) -> Result<(), ValidationError> {
    text.map_or(Ok(()), |text| length(field, text, min, max))
}

/// Checks that a list has at most `max` items.
pub(crate) fn count(field: &str, count: usize, max: usize) -> Result<(), ValidationError> {
    if count > max {
        return Err(ValidationError::new(
            field,
            format!("has {count} items; the maximum is {max}"),
        ));
    }
    Ok(())
}

/// Checks that a number is in the range `min` to `max`.
pub(crate) fn range(field: &str, value: u64, min: u64, max: u64) -> Result<(), ValidationError> {
    if value < min || value > max {
        return Err(ValidationError::new(
            field,
            format!("is {value}; it must be {min} to {max}"),
        ));
    }
    Ok(())
}

/// Checks that an optional number is in the range `min` to `max`.
pub(crate) fn optional_range(
    field: &str,
    value: Option<u64>,
    min: u64,
    max: u64,
) -> Result<(), ValidationError> {
    value.map_or(Ok(()), |value| range(field, value, min, max))
}

/// Gives the text of a `Patch` field when the request sets it.
pub(crate) fn patch_text(patch: &cordial_model::Patch<String>) -> Option<&str> {
    match patch {
        cordial_model::Patch::Set(text) => Some(text),
        cordial_model::Patch::Keep | cordial_model::Patch::Clear => None,
    }
}
