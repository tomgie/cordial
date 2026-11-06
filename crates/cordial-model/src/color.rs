//! RGB colors.

use std::fmt;

use serde::{Deserialize, Serialize};

/// An RGB color. Discord sends it as an integer, for example `0xFF0000` for red.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Color(pub u32);

#[allow(clippy::cast_possible_truncation)]
impl Color {
    /// The color that Discord treats as "no color" in roles and embeds.
    pub const DEFAULT: Self = Self(0);

    /// Makes a color from red, green, and blue parts.
    #[must_use]
    pub const fn from_rgb(red: u8, green: u8, blue: u8) -> Self {
        Self(((red as u32) << 16) | ((green as u32) << 8) | blue as u32)
    }

    /// Gives the red part.
    #[must_use]
    pub const fn red(self) -> u8 {
        (self.0 >> 16) as u8
    }

    /// Gives the green part.
    #[must_use]
    pub const fn green(self) -> u8 {
        (self.0 >> 8) as u8
    }

    /// Gives the blue part.
    #[must_use]
    pub const fn blue(self) -> u8 {
        self.0 as u8
    }
}

impl fmt::Debug for Color {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Color({self})")
    }
}

/// Formats the color as `#rrggbb`.
impl fmt::Display for Color {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "#{:06x}", self.0 & 0x00FF_FFFF)
    }
}

impl From<u32> for Color {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_formats() {
        let color = Color::from_rgb(0x12, 0x34, 0x56);

        assert_eq!(color.0, 0x0012_3456);
        assert_eq!(
            (color.red(), color.green(), color.blue()),
            (0x12, 0x34, 0x56)
        );
        assert_eq!(color.to_string(), "#123456");
    }
}
