//! Types for fields that can be absent, null, or set.
//!
//! The Discord docs use three states for some fields. `Option<T>` has only two.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A response field that can be absent, null, or a value.
///
/// Use it with `#[serde(default, skip_serializing_if = "Nullable::is_absent")]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Nullable<T> {
    /// The field is not in the payload.
    #[default]
    Absent,
    /// The field is in the payload with the value null.
    Null,
    /// The field has a value.
    Value(T),
}

impl<T> Nullable<T> {
    /// Tells if the field is not in the payload.
    #[must_use]
    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }

    /// Tells if the field is null.
    #[must_use]
    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Gives the value, or `None` when the field is absent or null.
    #[must_use]
    pub const fn as_ref(&self) -> Nullable<&T> {
        match self {
            Self::Absent => Nullable::Absent,
            Self::Null => Nullable::Null,
            Self::Value(value) => Nullable::Value(value),
        }
    }

    /// Gives the value, or `None` when the field is absent or null.
    #[must_use]
    pub fn value(self) -> Option<T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Absent | Self::Null => None,
        }
    }

    /// Gives the value as a reference, or `None` when the field is absent or null.
    #[must_use]
    pub const fn get(&self) -> Option<&T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Absent | Self::Null => None,
        }
    }
}

impl<T> From<Option<T>> for Nullable<T> {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Null, Self::Value)
    }
}

impl<T: Serialize> Serialize for Nullable<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => serializer.serialize_some(value),
            Self::Absent | Self::Null => serializer.serialize_none(),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Nullable<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(Self::from)
    }
}

/// A request field for a PATCH request: keep the current value, clear it, or set a new one.
///
/// Use it with `#[serde(default, skip_serializing_if = "Patch::is_keep")]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Patch<T> {
    /// Do not send the field. Discord keeps the current value.
    #[default]
    Keep,
    /// Send null. Discord clears the value.
    Clear,
    /// Send a new value.
    Set(T),
}

impl<T> Patch<T> {
    /// Tells if the field is not sent.
    #[must_use]
    pub const fn is_keep(&self) -> bool {
        matches!(self, Self::Keep)
    }
}

impl<T> From<Option<T>> for Patch<T> {
    /// Gives `Set` for `Some` and `Clear` for `None`.
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Clear, Self::Set)
    }
}

impl<T: Serialize> Serialize for Patch<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Set(value) => serializer.serialize_some(value),
            Self::Keep | Self::Clear => serializer.serialize_none(),
        }
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Patch<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(Self::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, Deserialize, Serialize, PartialEq)]
    struct Response {
        #[serde(default, skip_serializing_if = "Nullable::is_absent")]
        field: Nullable<u8>,
    }

    #[derive(Debug, Default, Serialize)]
    struct Request {
        #[serde(skip_serializing_if = "Patch::is_keep")]
        field: Patch<u8>,
    }

    #[test]
    fn nullable_keeps_three_states() {
        let absent: Response = serde_json::from_str("{}").unwrap();
        let null: Response = serde_json::from_str(r#"{"field":null}"#).unwrap();
        let value: Response = serde_json::from_str(r#"{"field":5}"#).unwrap();

        assert_eq!(absent.field, Nullable::Absent);
        assert_eq!(null.field, Nullable::Null);
        assert_eq!(value.field, Nullable::Value(5));
        assert_eq!(serde_json::to_string(&absent).unwrap(), "{}");
        assert_eq!(serde_json::to_string(&null).unwrap(), r#"{"field":null}"#);
    }

    #[test]
    fn patch_encodes_three_states() {
        let keep = Request { field: Patch::Keep };
        let clear = Request {
            field: Patch::Clear,
        };
        let set = Request {
            field: Patch::Set(5),
        };

        assert_eq!(serde_json::to_string(&keep).unwrap(), "{}");
        assert_eq!(serde_json::to_string(&clear).unwrap(), r#"{"field":null}"#);
        assert_eq!(serde_json::to_string(&set).unwrap(), r#"{"field":5}"#);
    }
}
