//! ISO 8601 timestamps.
//!
//! Discord sends times as ISO 8601 strings, for example `2021-04-21T19:01:11.123000+00:00`.
//! This module parses and formats them without an external date library.

use std::fmt;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize, Serializer};

const MICROS_PER_SECOND: i64 = 1_000_000;
const SECONDS_PER_DAY: i64 = 86_400;

/// A point in time with microsecond precision, in UTC.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    unix_micros: i64,
}

impl Timestamp {
    /// Makes a timestamp from Unix microseconds.
    #[must_use]
    pub const fn from_unix_micros(unix_micros: i64) -> Self {
        Self { unix_micros }
    }

    /// Makes a timestamp from Unix milliseconds.
    #[must_use]
    pub const fn from_unix_millis(unix_millis: i64) -> Self {
        Self::from_unix_micros(unix_millis.saturating_mul(1000))
    }

    /// Makes a timestamp from Unix seconds.
    #[must_use]
    pub const fn from_unix_seconds(unix_seconds: i64) -> Self {
        Self::from_unix_micros(unix_seconds.saturating_mul(MICROS_PER_SECOND))
    }

    /// Gives the current time.
    #[must_use]
    pub fn now() -> Self {
        Self::from(SystemTime::now())
    }

    /// Gives the time in Unix microseconds.
    #[must_use]
    pub const fn unix_micros(self) -> i64 {
        self.unix_micros
    }

    /// Gives the time in Unix milliseconds.
    #[must_use]
    pub const fn unix_millis(self) -> i64 {
        self.unix_micros.div_euclid(1000)
    }

    /// Gives the time in Unix seconds.
    #[must_use]
    pub const fn unix_seconds(self) -> i64 {
        self.unix_micros.div_euclid(MICROS_PER_SECOND)
    }

    /// Gives the duration from `earlier` to `self`, or `None` when `earlier` is later.
    #[must_use]
    pub fn duration_since(self, earlier: Self) -> Option<Duration> {
        let micros = self.unix_micros.checked_sub(earlier.unix_micros)?;
        u64::try_from(micros).ok().map(Duration::from_micros)
    }

    /// Parses an ISO 8601 (RFC 3339) text.
    ///
    /// # Errors
    ///
    /// Gives an error when the text has a wrong form or a value out of range.
    pub fn parse(text: &str) -> Result<Self, ParseTimestampError> {
        parse_rfc3339(text.as_bytes()).ok_or(ParseTimestampError)
    }

    /// Gives the parts of the date and time in UTC.
    #[must_use]
    pub fn to_parts(self) -> DateTimeParts {
        let seconds = self.unix_micros.div_euclid(MICROS_PER_SECOND);
        let micros = self.unix_micros.rem_euclid(MICROS_PER_SECOND);
        let days = seconds.div_euclid(SECONDS_PER_DAY);
        let second_of_day = seconds.rem_euclid(SECONDS_PER_DAY);
        let (year, month, day) = civil_from_days(days);

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        DateTimeParts {
            year,
            month,
            day,
            hour: (second_of_day / 3600) as u8,
            minute: (second_of_day % 3600 / 60) as u8,
            second: (second_of_day % 60) as u8,
            microsecond: micros as u32,
        }
    }
}

/// The parts of a date and time in UTC.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct DateTimeParts {
    /// The year.
    pub year: i64,
    /// The month, 1 to 12.
    pub month: u8,
    /// The day of the month, 1 to 31.
    pub day: u8,
    /// The hour, 0 to 23.
    pub hour: u8,
    /// The minute, 0 to 59.
    pub minute: u8,
    /// The second, 0 to 59.
    pub second: u8,
    /// The microsecond, 0 to 999,999.
    pub microsecond: u32,
}

/// The error when a text is not a valid ISO 8601 timestamp.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseTimestampError;

impl fmt::Display for ParseTimestampError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the text is not a valid ISO 8601 timestamp")
    }
}

impl std::error::Error for ParseTimestampError {}

impl fmt::Debug for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Timestamp({self})")
    }
}

/// Formats the time as Discord does: `YYYY-MM-DDTHH:MM:SS.ffffff+00:00`.
impl fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts = self.to_parts();
        write!(
            formatter,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}+00:00",
            parts.year,
            parts.month,
            parts.day,
            parts.hour,
            parts.minute,
            parts.second,
            parts.microsecond
        )
    }
}

/// Gives the time that is `duration` later. Saturates at the limits.
impl std::ops::Add<Duration> for Timestamp {
    type Output = Self;

    fn add(self, duration: Duration) -> Self {
        let micros = i64::try_from(duration.as_micros()).unwrap_or(i64::MAX);
        Self::from_unix_micros(self.unix_micros.saturating_add(micros))
    }
}

impl FromStr for Timestamp {
    type Err = ParseTimestampError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl From<SystemTime> for Timestamp {
    fn from(time: SystemTime) -> Self {
        let micros = match time.duration_since(UNIX_EPOCH) {
            Ok(after) => i64::try_from(after.as_micros()).unwrap_or(i64::MAX),
            Err(error) => {
                i64::try_from(error.duration().as_micros()).map_or(i64::MIN, |value| -value)
            }
        };
        Self::from_unix_micros(micros)
    }
}

impl From<Timestamp> for SystemTime {
    fn from(timestamp: Timestamp) -> Self {
        let magnitude = Duration::from_micros(timestamp.unix_micros.unsigned_abs());
        if timestamp.unix_micros >= 0 {
            UNIX_EPOCH + magnitude
        } else {
            UNIX_EPOCH - magnitude
        }
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(TimestampVisitor)
    }
}

struct TimestampVisitor;

impl Visitor<'_> for TimestampVisitor {
    type Value = Timestamp;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an ISO 8601 timestamp")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Timestamp, E> {
        Timestamp::parse(value)
            .map_err(|_| E::custom(format_args!("{value:?} is not a valid ISO 8601 timestamp")))
    }
}

/// Serde functions for a time that Discord sends as Unix seconds (a JSON number).
pub mod unix_seconds {
    use super::{Deserialize, Deserializer, Serializer, Timestamp};

    /// Serializes the time as Unix seconds.
    ///
    /// # Errors
    ///
    /// Gives the error of the serializer.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S: Serializer>(value: &Timestamp, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(value.unix_seconds())
    }

    /// Deserializes the time from Unix seconds.
    ///
    /// # Errors
    ///
    /// Gives the error of the deserializer.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Timestamp, D::Error> {
        i64::deserialize(deserializer).map(Timestamp::from_unix_seconds)
    }
}

/// Serde functions for a time that Discord sends as Unix milliseconds (a JSON number).
pub mod unix_millis {
    use super::{Deserialize, Deserializer, Serializer, Timestamp};

    /// Serializes the time as Unix milliseconds.
    ///
    /// # Errors
    ///
    /// Gives the error of the serializer.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn serialize<S: Serializer>(value: &Timestamp, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(value.unix_millis())
    }

    /// Deserializes the time from Unix milliseconds.
    ///
    /// # Errors
    ///
    /// Gives the error of the deserializer.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Timestamp, D::Error> {
        i64::deserialize(deserializer).map(Timestamp::from_unix_millis)
    }
}

fn parse_rfc3339(text: &[u8]) -> Option<Timestamp> {
    let mut cursor = Cursor { text, position: 0 };

    let year = cursor.digits(4)?;
    cursor.expect(b'-')?;
    let month = cursor.digits(2)?;
    cursor.expect(b'-')?;
    let day = cursor.digits(2)?;
    cursor.expect_one_of(b"Tt ")?;
    let hour = cursor.digits(2)?;
    cursor.expect(b':')?;
    let minute = cursor.digits(2)?;
    cursor.expect(b':')?;
    let second = cursor.digits(2)?;

    let mut micros = 0;
    if cursor.peek() == Some(b'.') {
        cursor.position += 1;
        micros = cursor.fraction_as_micros()?;
    }

    let offset_seconds = cursor.offset()?;
    if cursor.position != text.len() {
        return None;
    }

    let valid_date = (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month);
    let valid_time = hour <= 23 && minute <= 59 && second <= 60;
    if !valid_date || !valid_time {
        return None;
    }

    let days = days_from_civil(year, month, day);
    let seconds = days * SECONDS_PER_DAY + hour * 3600 + minute * 60 + second - offset_seconds;
    seconds
        .checked_mul(MICROS_PER_SECOND)
        .and_then(|value| value.checked_add(micros))
        .map(Timestamp::from_unix_micros)
}

struct Cursor<'a> {
    text: &'a [u8],
    position: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<u8> {
        self.text.get(self.position).copied()
    }

    fn expect(&mut self, expected: u8) -> Option<()> {
        self.expect_one_of(&[expected])
    }

    fn expect_one_of(&mut self, expected: &[u8]) -> Option<()> {
        let byte = self.peek()?;
        if !expected.contains(&byte) {
            return None;
        }
        self.position += 1;
        Some(())
    }

    fn digits(&mut self, count: usize) -> Option<i64> {
        let mut value = 0;
        for _ in 0..count {
            let byte = self.peek()?;
            if !byte.is_ascii_digit() {
                return None;
            }
            value = value * 10 + i64::from(byte - b'0');
            self.position += 1;
        }
        Some(value)
    }

    /// Reads one or more fraction digits. Digits after the sixth are dropped.
    fn fraction_as_micros(&mut self) -> Option<i64> {
        let mut value = 0;
        let mut count = 0;
        while let Some(byte) = self.peek().filter(u8::is_ascii_digit) {
            if count < 6 {
                value = value * 10 + i64::from(byte - b'0');
            }
            count += 1;
            self.position += 1;
        }
        if count == 0 {
            return None;
        }
        for _ in count..6 {
            value *= 10;
        }
        Some(value)
    }

    fn offset(&mut self) -> Option<i64> {
        match self.peek()? {
            b'Z' | b'z' => {
                self.position += 1;
                Some(0)
            }
            sign @ (b'+' | b'-') => {
                self.position += 1;
                let hours = self.digits(2)?;
                self.expect(b':')?;
                let minutes = self.digits(2)?;
                if hours > 23 || minutes > 59 {
                    return None;
                }
                let seconds = hours * 3600 + minutes * 60;
                Some(if sign == b'+' { seconds } else { -seconds })
            }
            _ => None,
        }
    }
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 for a date in the proleptic Gregorian calendar.
///
/// The algorithm is from Howard Hinnant, "chrono-Compatible Low-Level Date Algorithms".
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month_index = (month + 9) % 12;
    let day_of_year = (153 * month_index + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The inverse of [`days_from_civil`].
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn civil_from_days(days: i64) -> (i64, u8, u8) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month as u8, day as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_discord_format() {
        let timestamp = Timestamp::parse("2021-04-21T19:01:11.123000+00:00").unwrap();

        assert_eq!(timestamp.unix_millis(), 1_619_031_671_123);
        assert_eq!(timestamp.to_string(), "2021-04-21T19:01:11.123000+00:00");
    }

    #[test]
    fn parses_other_forms() {
        let base = Timestamp::parse("2021-04-21T19:01:11Z").unwrap();

        assert_eq!(base.unix_seconds(), 1_619_031_671);
        assert_eq!(Timestamp::parse("2021-04-21T21:01:11+02:00").unwrap(), base);
        assert_eq!(Timestamp::parse("2021-04-21T17:31:11-01:30").unwrap(), base);
        assert_eq!(
            Timestamp::parse("2021-04-21T19:01:11.1234567Z")
                .unwrap()
                .unix_micros(),
            base.unix_micros() + 123_456
        );
    }

    #[test]
    fn refuses_bad_text() {
        for text in [
            "",
            "2021-04-21",
            "2021-13-01T00:00:00Z",
            "2021-02-29T00:00:00Z",
            "2021-04-21T24:00:00Z",
            "2021-04-21T19:01:11",
            "2021-04-21T19:01:11.Z",
            "2021-04-21T19:01:11Z ",
        ] {
            assert!(Timestamp::parse(text).is_err(), "{text:?}");
        }
    }

    #[test]
    fn handles_leap_day_and_times_before_1970() {
        let leap = Timestamp::parse("2024-02-29T12:00:00Z").unwrap();
        let early = Timestamp::parse("1969-12-31T23:59:59.500000+00:00").unwrap();

        assert_eq!(leap.to_string(), "2024-02-29T12:00:00.000000+00:00");
        assert_eq!(early.unix_micros(), -500_000);
        assert_eq!(early.to_string(), "1969-12-31T23:59:59.500000+00:00");
    }

    #[test]
    fn round_trips_days_over_a_long_range() {
        for days in (-800_000..800_000).step_by(997) {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(
                days_from_civil(year, i64::from(month), i64::from(day)),
                days
            );
        }
    }

    #[test]
    fn uses_serde_string_form() {
        let timestamp: Timestamp = serde_json::from_str("\"2015-01-01T00:00:00+00:00\"").unwrap();

        assert_eq!(timestamp.unix_millis(), 1_420_070_400_000);
        assert_eq!(
            serde_json::to_string(&timestamp).unwrap(),
            "\"2015-01-01T00:00:00.000000+00:00\""
        );
    }
}
