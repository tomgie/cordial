//! Serde helpers for Discord encodings.

use std::fmt;
use std::marker::PhantomData;
use std::str::FromStr;

use serde::de::{self, Deserializer, IgnoredAny, Visitor};
use serde::{Deserialize, Serializer};

/// Deserializes an integer that Discord sends as a JSON string or as a JSON number.
///
/// # Errors
///
/// Gives an error when the value is not an integer or does not fit in `T`.
pub fn deserialize_string_or_integer<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr + TryFrom<u64> + TryFrom<i64>,
{
    deserializer.deserialize_any(StringOrIntegerVisitor(PhantomData))
}

struct StringOrIntegerVisitor<T>(PhantomData<T>);

impl<T> Visitor<'_> for StringOrIntegerVisitor<T>
where
    T: FromStr + TryFrom<u64> + TryFrom<i64>,
{
    type Value = T;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an integer or a string that holds an integer")
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<T, E> {
        T::try_from(value).map_err(|_| E::custom(format_args!("integer {value} is out of range")))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<T, E> {
        T::try_from(value).map_err(|_| E::custom(format_args!("integer {value} is out of range")))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<T, E> {
        value
            .parse()
            .map_err(|_| E::custom(format_args!("string {value:?} is not a valid integer")))
    }
}

/// Serde functions for an integer that Discord sends as a JSON string.
///
/// Use it with `#[serde(with = "crate::serde_util::string_integer")]`.
pub mod string_integer {
    use super::{Deserializer, FromStr, Serializer};

    /// Serializes the integer as a JSON string.
    ///
    /// # Errors
    ///
    /// Gives the error of the serializer.
    pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: std::fmt::Display,
    {
        serializer.collect_str(value)
    }

    /// Deserializes the integer from a JSON string or a JSON number.
    ///
    /// # Errors
    ///
    /// Gives an error when the value is not an integer or does not fit in `T`.
    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
        T: FromStr + TryFrom<u64> + TryFrom<i64>,
    {
        super::deserialize_string_or_integer(deserializer)
    }
}

/// Serde functions for an optional integer that Discord sends as a JSON string.
pub mod option_string_integer {
    use super::{Deserialize, Deserializer, FromStr, Serializer};

    /// Serializes the integer as a JSON string, or `None` as null.
    ///
    /// # Errors
    ///
    /// Gives the error of the serializer.
    #[allow(clippy::ref_option)]
    pub fn serialize<S, T>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: std::fmt::Display,
    {
        match value {
            Some(value) => serializer.collect_str(value),
            None => serializer.serialize_none(),
        }
    }

    /// Deserializes the integer from a JSON string, a JSON number, or null.
    ///
    /// # Errors
    ///
    /// Gives an error when the value is not an integer or does not fit in `T`.
    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: FromStr + TryFrom<u64> + TryFrom<i64>,
    {
        #[derive(Deserialize)]
        #[serde(transparent)]
        struct Wrapper<T>(#[serde(deserialize_with = "super::deserialize_string_or_integer")] T)
        where
            T: FromStr + TryFrom<u64> + TryFrom<i64>;

        let value: Option<Wrapper<T>> = Option::deserialize(deserializer)?;
        Ok(value.map(|Wrapper(inner)| inner))
    }
}

/// Serde functions for a field where "present" means `true`, also when the value is null.
///
/// Discord uses this form in role tags. Use it with
/// `#[serde(default, skip_serializing_if = "std::ops::Not::not", with = "crate::serde_util::present_is_true")]`.
pub mod present_is_true {
    use super::{Deserialize, Deserializer, IgnoredAny, Serializer};

    /// Serializes `true` as null. The field must be skipped when it is `false`.
    ///
    /// # Errors
    ///
    /// Gives the error of the serializer.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S: Serializer>(_value: &bool, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_none()
    }

    /// Gives `true` for any value. Serde calls this only when the field is present.
    ///
    /// # Errors
    ///
    /// Gives the error of the deserializer.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
        IgnoredAny::deserialize(deserializer)?;
        Ok(true)
    }
}

/// Tells if a value is equal to the default value. Use it with `skip_serializing_if`.
pub fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// Tells if a boolean is `false`. Use it with `skip_serializing_if`.
#[allow(clippy::trivially_copy_pass_by_ref)]
#[must_use]
pub fn is_false(value: &bool) -> bool {
    !*value
}
