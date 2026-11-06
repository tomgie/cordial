//! Macros that generate the enum and flag types of the model.
//!
//! Discord adds enum values and flag bits without a new API version.
//! These macros make every generated type keep values that it does not know.

/// Generates an integer enum with an `Unknown` variant.
///
/// Two values are equal when their raw values are equal. Thus `Unknown(0)` equals the
/// variant with the value 0.
#[macro_export]
#[doc(hidden)]
macro_rules! discord_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident: $repr:ty {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident = $value:literal,
            )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug)]
        #[non_exhaustive]
        $vis enum $name {
            $(
                $(#[$variant_meta])*
                $variant,
            )*
            /// A value that this version of the library does not know.
            Unknown($repr),
        }

        impl $name {
            /// Gives the raw value that Discord uses.
            #[must_use]
            pub const fn value(self) -> $repr {
                match self {
                    $( Self::$variant => $value, )*
                    Self::Unknown(value) => value,
                }
            }

            /// Makes a value from the raw value that Discord uses.
            #[must_use]
            pub const fn from_value(value: $repr) -> Self {
                match value {
                    $( $value => Self::$variant, )*
                    other => Self::Unknown(other),
                }
            }

            /// Tells if this version of the library does not know the value.
            #[must_use]
            pub const fn is_unknown(self) -> bool {
                matches!(Self::from_value(self.value()), Self::Unknown(_))
            }
        }

        impl ::core::cmp::PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.value() == other.value()
            }
        }

        impl ::core::cmp::Eq for $name {}

        impl ::core::hash::Hash for $name {
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                self.value().hash(state);
            }
        }

        /// The default is the value 0, as Discord treats an absent integer.
        impl ::core::default::Default for $name {
            fn default() -> Self {
                Self::from_value(0)
            }
        }

        impl ::core::convert::From<$repr> for $name {
            fn from(value: $repr) -> Self {
                Self::from_value(value)
            }
        }

        impl ::core::convert::From<$name> for $repr {
            fn from(value: $name) -> Self {
                value.value()
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                ::serde::Serialize::serialize(&self.value(), serializer)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <$repr as ::serde::Deserialize<'de>>::deserialize(deserializer)?;
                Ok(Self::from_value(value))
            }
        }
    };
}

/// Generates a string enum with an `Unknown` variant.
#[macro_export]
#[doc(hidden)]
macro_rules! discord_string_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $(
                $(#[$variant_meta:meta])*
                $variant:ident = $value:literal,
            )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug)]
        #[non_exhaustive]
        $vis enum $name {
            $(
                $(#[$variant_meta])*
                $variant,
            )*
            /// A value that this version of the library does not know.
            Unknown(::std::string::String),
        }

        impl $name {
            /// Gives the raw value that Discord uses.
            #[must_use]
            pub fn as_str(&self) -> &str {
                match self {
                    $( Self::$variant => $value, )*
                    Self::Unknown(value) => value,
                }
            }

            /// Makes a value from the raw value that Discord uses.
            #[must_use]
            pub fn from_value(value: &str) -> Self {
                match value {
                    $( $value => Self::$variant, )*
                    other => Self::Unknown(other.to_owned()),
                }
            }

            /// Tells if this version of the library does not know the value.
            #[must_use]
            pub fn is_unknown(&self) -> bool {
                matches!(Self::from_value(self.as_str()), Self::Unknown(_))
            }
        }

        impl ::core::cmp::PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.as_str() == other.as_str()
            }
        }

        impl ::core::cmp::Eq for $name {}

        impl ::core::hash::Hash for $name {
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
                self.as_str().hash(state);
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl ::core::convert::From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::from_value(value)
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <::std::borrow::Cow<'de, str> as ::serde::Deserialize<'de>>::deserialize(deserializer)?;
                Ok(Self::from_value(&value))
            }
        }
    };
}

/// Generates a bit flag type that keeps unknown bits.
///
/// The serde form is `integer` (a JSON number) or `string` (a decimal string, as for permissions).
#[macro_export]
#[doc(hidden)]
macro_rules! discord_flags {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident: $repr:ty as $form:ident {
            $(
                $(#[$flag_meta:meta])*
                const $flag:ident = $bits:expr;
            )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
        $vis struct $name {
            bits: $repr,
        }

        #[allow(clippy::unreadable_literal)]
        impl $name {
            $(
                $(#[$flag_meta])*
                pub const $flag: Self = Self { bits: $bits };
            )*

            /// All flags that this version of the library knows, with their names.
            pub const FLAGS: &'static [(&'static str, Self)] = &[
                $( (stringify!($flag), Self::$flag), )*
            ];

            /// Gives a value with no flags.
            #[must_use]
            pub const fn empty() -> Self {
                Self { bits: 0 }
            }

            /// Gives a value with all known flags.
            #[must_use]
            pub const fn all() -> Self {
                Self { bits: 0 $( | $bits )* }
            }

            /// Makes a value from raw bits. Unknown bits stay.
            #[must_use]
            pub const fn from_bits_retain(bits: $repr) -> Self {
                Self { bits }
            }

            /// Gives the raw bits, unknown bits included.
            #[must_use]
            pub const fn bits(self) -> $repr {
                self.bits
            }

            /// Gives the bits that this version of the library does not know.
            #[must_use]
            pub const fn unknown_bits(self) -> $repr {
                self.bits & !Self::all().bits
            }

            /// Tells if no bit is set.
            #[must_use]
            pub const fn is_empty(self) -> bool {
                self.bits == 0
            }

            /// Tells if all bits of `other` are set.
            #[must_use]
            pub const fn contains(self, other: Self) -> bool {
                self.bits & other.bits == other.bits
            }

            /// Tells if one or more bits of `other` are set.
            #[must_use]
            pub const fn intersects(self, other: Self) -> bool {
                self.bits & other.bits != 0
            }

            /// Gives the bits that are set in `self` or `other`.
            #[must_use]
            pub const fn union(self, other: Self) -> Self {
                Self { bits: self.bits | other.bits }
            }

            /// Gives the bits that are set in both `self` and `other`.
            #[must_use]
            pub const fn intersection(self, other: Self) -> Self {
                Self { bits: self.bits & other.bits }
            }

            /// Gives the bits of `self` that are not set in `other`.
            #[must_use]
            pub const fn difference(self, other: Self) -> Self {
                Self { bits: self.bits & !other.bits }
            }

            /// Sets the bits of `other`.
            pub const fn insert(&mut self, other: Self) {
                self.bits |= other.bits;
            }

            /// Clears the bits of `other`.
            pub const fn remove(&mut self, other: Self) {
                self.bits &= !other.bits;
            }

            /// Sets or clears the bits of `other`.
            pub const fn set(&mut self, other: Self, value: bool) {
                if value {
                    self.insert(other);
                } else {
                    self.remove(other);
                }
            }

            /// Gives the names of the known flags that are set.
            pub fn names(self) -> impl Iterator<Item = &'static str> {
                Self::FLAGS
                    .iter()
                    .filter(move |(_, flag)| !flag.is_empty() && self.contains(*flag))
                    .map(|(name, _)| *name)
            }
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                formatter.write_str(concat!(stringify!($name), "("))?;
                let mut first = true;
                for name in self.names() {
                    if !first {
                        formatter.write_str(" | ")?;
                    }
                    formatter.write_str(name)?;
                    first = false;
                }
                let unknown = self.unknown_bits();
                if unknown != 0 {
                    if !first {
                        formatter.write_str(" | ")?;
                    }
                    write!(formatter, "{unknown:#x}")?;
                    first = false;
                }
                if first {
                    formatter.write_str("empty")?;
                }
                formatter.write_str(")")
            }
        }

        impl ::core::ops::BitOr for $name {
            type Output = Self;

            fn bitor(self, other: Self) -> Self {
                self.union(other)
            }
        }

        impl ::core::ops::BitOrAssign for $name {
            fn bitor_assign(&mut self, other: Self) {
                self.insert(other);
            }
        }

        impl ::core::ops::BitAnd for $name {
            type Output = Self;

            fn bitand(self, other: Self) -> Self {
                self.intersection(other)
            }
        }

        impl ::core::ops::BitAndAssign for $name {
            fn bitand_assign(&mut self, other: Self) {
                self.bits &= other.bits;
            }
        }

        impl ::core::ops::Sub for $name {
            type Output = Self;

            fn sub(self, other: Self) -> Self {
                self.difference(other)
            }
        }

        impl ::core::ops::SubAssign for $name {
            fn sub_assign(&mut self, other: Self) {
                self.remove(other);
            }
        }

        impl ::core::iter::FromIterator<$name> for $name {
            fn from_iter<I: IntoIterator<Item = $name>>(iterator: I) -> Self {
                iterator.into_iter().fold(Self::empty(), Self::union)
            }
        }

        impl ::core::iter::Extend<$name> for $name {
            fn extend<I: IntoIterator<Item = $name>>(&mut self, iterator: I) {
                for flag in iterator {
                    self.insert(flag);
                }
            }
        }

        $crate::discord_flags!(@serde $name, $repr, $form);
    };

    (@serde $name:ident, $repr:ty, integer) => {
        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                ::serde::Serialize::serialize(&self.bits, serializer)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let bits = <$repr as ::serde::Deserialize<'de>>::deserialize(deserializer)?;
                Ok(Self::from_bits_retain(bits))
            }
        }
    };

    (@serde $name:ident, $repr:ty, string) => {
        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(&self.bits)
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let bits = $crate::serde_util::deserialize_string_or_integer::<D, $repr>(deserializer)?;
                Ok(Self::from_bits_retain(bits))
            }
        }
    };
}
