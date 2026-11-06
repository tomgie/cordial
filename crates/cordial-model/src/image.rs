//! Image hashes for CDN assets.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize, Serializer};

/// The hash of an image on the Discord CDN, for example an avatar or an icon.
///
/// A hash with the prefix `a_` is animated. Most hashes are 32 hex digits. The type stores
/// them in 17 bytes. Other forms are stored as text.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ImageHash(Repr);

#[derive(Clone, PartialEq, Eq, Hash)]
enum Repr {
    Hex { bytes: [u8; 16], animated: bool },
    Text(Box<str>),
}

impl ImageHash {
    /// Makes a hash from text. Any text is accepted.
    #[must_use]
    pub fn new(text: &str) -> Self {
        let (animated, hex) = match text.strip_prefix("a_") {
            Some(rest) => (true, rest),
            None => (false, text),
        };

        match parse_lower_hex(hex) {
            Some(bytes) => Self(Repr::Hex { bytes, animated }),
            None => Self(Repr::Text(text.into())),
        }
    }

    /// Tells if the image is animated (the hash starts with `a_`).
    #[must_use]
    pub fn is_animated(&self) -> bool {
        match &self.0 {
            Repr::Hex { animated, .. } => *animated,
            Repr::Text(text) => text.starts_with("a_"),
        }
    }
}

fn parse_lower_hex(text: &str) -> Option<[u8; 16]> {
    let digits = text.as_bytes();
    if digits.len() != 32 {
        return None;
    }

    let mut bytes = [0; 16];
    for (index, pair) in digits.as_chunks::<2>().0.iter().enumerate() {
        bytes[index] = (lower_hex_value(pair[0])? << 4) | lower_hex_value(pair[1])?;
    }
    Some(bytes)
}

// Upper case digits go to the text form, so that the text stays exactly the same.
fn lower_hex_value(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        _ => None,
    }
}

impl fmt::Display for ImageHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Repr::Hex { bytes, animated } => {
                if *animated {
                    formatter.write_str("a_")?;
                }
                for byte in bytes {
                    write!(formatter, "{byte:02x}")?;
                }
                Ok(())
            }
            Repr::Text(text) => formatter.write_str(text),
        }
    }
}

impl fmt::Debug for ImageHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ImageHash({self})")
    }
}

impl FromStr for ImageHash {
    type Err = std::convert::Infallible;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(text))
    }
}

impl Serialize for ImageHash {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ImageHash {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(ImageHashVisitor)
    }
}

struct ImageHashVisitor;

impl Visitor<'_> for ImageHashVisitor {
    type Value = ImageHash;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an image hash string")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<ImageHash, E> {
        Ok(ImageHash::new(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_text_exactly() {
        for text in [
            "8342729096ea3675442027381ff50dfe",
            "a_8342729096ea3675442027381ff50dfe",
            "8342729096EA3675442027381FF50DFE",
            "nameplates/twilight/",
            "",
        ] {
            assert_eq!(ImageHash::new(text).to_string(), text);
        }
    }

    #[test]
    fn detects_animation() {
        assert!(ImageHash::new("a_8342729096ea3675442027381ff50dfe").is_animated());
        assert!(!ImageHash::new("8342729096ea3675442027381ff50dfe").is_animated());
    }
}
