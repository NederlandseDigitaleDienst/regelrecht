//! A calendar date as a value (RFC-055).
//!
//! A [`Date`] is a day of the Gregorian calendar in the years 0001 to 9999,
//! without a time or a time zone. It is held as its canonical text,
//! `YYYY-MM-DD`, which is also how it is written: a date leaves the engine
//! as the same string it had before dates were a type of their own. The text
//! is private and checked on construction, so a `Date` is always a real day.

use std::fmt;
use std::str::FromStr;

/// A day of the Gregorian calendar, in the years 0001 to 9999.
///
/// Dates order by time: the canonical text is zero-padded, so comparing two
/// texts compares the days.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date(String);

/// A text that is not a date in the form `YYYY-MM-DD`, or names no real day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDateError(String);

impl fmt::Display for ParseDateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "'{}' is not a date: expected a calendar day as YYYY-MM-DD",
            self.0
        )
    }
}

impl std::error::Error for ParseDateError {}

/// The number of days in `month` of `year`, or `None` for a month that does
/// not exist.
fn days_in_month(year: u32, month: u32) -> Option<u32> {
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => Some(31),
        4 | 6 | 9 | 11 => Some(30),
        2 => Some(if leap { 29 } else { 28 }),
        _ => None,
    }
}

impl Date {
    /// The date with this year, month and day. `None` when no such day
    /// exists (month 13, 30 February) or the year is outside 0001 to 9999.
    pub fn from_ymd(year: u32, month: u32, day: u32) -> Option<Self> {
        if !(1..=9999).contains(&year) {
            return None;
        }
        let last = days_in_month(year, month)?;
        (1..=last)
            .contains(&day)
            .then(|| Date(format!("{year:04}-{month:02}-{day:02}")))
    }

    /// The canonical text, `YYYY-MM-DD`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The year, 1 to 9999.
    pub fn year(&self) -> u32 {
        self.part(0..4)
    }

    /// The month, 1 to 12.
    pub fn month(&self) -> u32 {
        self.part(5..7)
    }

    /// The day of the month, 1 to 31.
    pub fn day(&self) -> u32 {
        self.part(8..10)
    }

    /// One numeric part of the text. The text is ASCII digits and dashes in
    /// fixed positions by construction, so the range always holds digits.
    fn part(&self, range: std::ops::Range<usize>) -> u32 {
        self.0
            .get(range)
            .and_then(|digits| digits.parse().ok())
            .unwrap_or_default()
    }

    /// Whether `text` is a date in canonical form, without building one.
    pub fn is_date(text: &str) -> bool {
        text.parse::<Date>().is_ok()
    }
}

/// Reads the canonical form only: four, two and two digits with dashes
/// between. `2025-1-5` and `2025-02-30` are refused.
impl FromStr for Date {
    type Err = ParseDateError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let fail = || ParseDateError(text.to_string());
        let bytes = text.as_bytes();
        let digits = |range: std::ops::Range<usize>| -> Option<u32> {
            let part = text.get(range)?;
            part.bytes()
                .all(|b| b.is_ascii_digit())
                .then(|| part.parse().ok())
                .flatten()
        };
        if bytes.len() != 10 || bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
            return Err(fail());
        }
        let (year, month, day) = (digits(0..4), digits(5..7), digits(8..10));
        match (year, month, day) {
            (Some(year), Some(month), Some(day)) => {
                Date::from_ymd(year, month, day).ok_or_else(fail)
            }
            _ => Err(fail()),
        }
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> Date {
        text.parse().expect("test literal should be a date")
    }

    #[test]
    fn the_canonical_form_is_read_and_written_back() {
        for text in ["2025-01-01", "0001-01-01", "9999-12-31", "2024-02-29"] {
            assert_eq!(date(text).as_str(), text);
            assert_eq!(date(text).to_string(), text);
        }
    }

    #[test]
    fn a_text_that_is_no_calendar_day_is_refused() {
        for text in [
            "2025-02-30",
            "2025-13-01",
            "2025-00-10",
            "2025-01-00",
            "2023-02-29",
            "1900-02-29",
            "0000-01-01",
            "2025-1-5",
            "2025/01/05",
            "20250105",
            "2025-01-05T00:00",
            " 2025-01-05",
            "2025-01-0５",
            "",
        ] {
            assert!(text.parse::<Date>().is_err(), "{text:?} should not parse");
        }
    }

    #[test]
    fn leap_years_follow_the_gregorian_rule() {
        assert!(Date::from_ymd(2000, 2, 29).is_some());
        assert!(Date::from_ymd(2024, 2, 29).is_some());
        assert!(Date::from_ymd(1900, 2, 29).is_none());
        assert!(Date::from_ymd(2025, 2, 29).is_none());
    }

    #[test]
    fn the_parts_are_read_from_the_text() {
        let day = date("2025-03-09");
        assert_eq!((day.year(), day.month(), day.day()), (2025, 3, 9));
        assert_eq!(
            Date::from_ymd(987, 6, 5).map(|d| d.to_string()),
            Some("0987-06-05".into())
        );
    }

    #[test]
    fn dates_order_by_time() {
        assert!(date("2024-12-31") < date("2025-01-01"));
        assert!(date("0999-12-31") < date("1000-01-01"));
        assert!(date("2025-02-01") > date("2025-01-31"));
    }

    #[test]
    fn years_outside_the_range_are_refused() {
        assert!(Date::from_ymd(0, 1, 1).is_none());
        assert!(Date::from_ymd(10_000, 1, 1).is_none());
    }
}
