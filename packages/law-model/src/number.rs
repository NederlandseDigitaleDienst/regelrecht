//! Exact numbers (RFC-055).
//!
//! A [`Number`] is a rational number held as a fraction of two arbitrarily
//! large integers, so no arithmetic operation rounds: `(1 / 3) * 3` is `1`.
//! Rounding happens only where a rulework asks for it, through
//! [`Number::round_to`].
//!
//! The fraction is private. Everything outside this module goes through the
//! methods here, which keeps the arithmetic library replaceable.

use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};
use std::str::FromStr;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Pow, Signed, ToPrimitive, Zero};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// An exact rational number.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Number(BigRational);

/// How [`Number::round_to`] treats the digits it drops (RFC-024).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
    /// To the nearest, a tie away from zero: `-2.5` gives `-3`.
    HalfAwayFromZero,
    /// Toward positive infinity.
    Ceil,
    /// Toward negative infinity.
    Floor,
}

/// A text that is not a number in decimal or fraction notation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseNumberError(String);

impl fmt::Display for ParseNumberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "'{}' is not a number", self.0)
    }
}

impl std::error::Error for ParseNumberError {}

/// The largest exponent [`Number::from_str`] accepts in `1e400` notation. It
/// covers every `f64` and keeps a short text from asking for a huge number.
const MAX_PARSE_EXPONENT: u32 = 400;

fn ten_to(exponent: u32) -> BigInt {
    Pow::pow(BigInt::from(10), exponent)
}

impl Number {
    /// The number zero.
    pub fn zero() -> Self {
        Number(BigRational::zero())
    }

    /// The number one.
    pub fn one() -> Self {
        Number(BigRational::one())
    }

    /// The shortest decimal that gives this double, as an exact number: `0.1`
    /// becomes one tenth and not the binary fraction nearest to it. `None`
    /// for NaN and the infinities.
    pub fn from_f64(value: f64) -> Option<Self> {
        if !value.is_finite() {
            return None;
        }
        // `Display` for f64 prints the shortest digits that read back as the
        // same double, always in plain decimal notation.
        value.to_string().parse().ok()
    }

    /// The nearest double. `None` when the number is too large for one.
    pub fn to_f64(&self) -> Option<f64> {
        self.0.to_f64().filter(|f| f.is_finite())
    }

    /// The number as an `i64`. `None` when it has a fraction or does not fit.
    pub fn to_i64(&self) -> Option<i64> {
        if self.0.is_integer() {
            self.0.numer().to_i64()
        } else {
            None
        }
    }

    /// Whether the number is zero.
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    /// Whether the number has no fraction.
    pub fn is_integer(&self) -> bool {
        self.0.is_integer()
    }

    /// Whether the number is below zero.
    pub fn is_negative(&self) -> bool {
        self.0.is_negative()
    }

    /// The number without its sign.
    pub fn abs(&self) -> Self {
        Number(self.0.abs())
    }

    /// The number without its fraction, toward zero: `-2.7` gives `-2`.
    pub fn trunc(&self) -> Self {
        Number(self.0.trunc())
    }

    /// This number divided by `divisor`, exactly. `None` when `divisor` is
    /// zero.
    pub fn checked_div(&self, divisor: &Number) -> Option<Self> {
        if divisor.is_zero() {
            None
        } else {
            Some(Number(&self.0 / &divisor.0))
        }
    }

    /// The number rounded to `decimals` decimals. A negative `decimals`
    /// rounds to tens or hundreds: `-2` rounds to whole hundreds.
    pub fn round_to(&self, decimals: i32, mode: Rounding) -> Self {
        let scale = BigRational::from_integer(ten_to(decimals.unsigned_abs()));
        let scaled = if decimals >= 0 {
            &self.0 * &scale
        } else {
            &self.0 / &scale
        };
        let whole = match mode {
            Rounding::HalfAwayFromZero => scaled.round(),
            Rounding::Ceil => scaled.ceil(),
            Rounding::Floor => scaled.floor(),
        };
        Number(if decimals >= 0 {
            whole / scale
        } else {
            whole * scale
        })
    }

    /// The number of decimals needed to write the number exactly, or `None`
    /// when no finite decimal does, as for one third.
    pub fn decimal_places(&self) -> Option<u32> {
        let mut rest = self.0.denom().clone();
        let mut twos = 0u32;
        let mut fives = 0u32;
        let two = BigInt::from(2);
        let five = BigInt::from(5);
        while (&rest % &two).is_zero() {
            rest /= &two;
            twos += 1;
        }
        while (&rest % &five).is_zero() {
            rest /= &five;
            fives += 1;
        }
        rest.is_one().then_some(twos.max(fives))
    }

    /// The size of the number in bits: the larger of its numerator and its
    /// denominator. The engine holds this against its resource limit.
    pub fn bits(&self) -> u64 {
        self.0.numer().bits().max(self.0.denom().bits())
    }

    /// The exact fraction notation, `numerator/denominator`, which
    /// [`Number::from_str`] reads back. An integer is written without a
    /// denominator.
    pub fn to_fraction_string(&self) -> String {
        if self.0.is_integer() {
            self.0.numer().to_string()
        } else {
            format!("{}/{}", self.0.numer(), self.0.denom())
        }
    }

    /// Write the number with exactly `decimals` decimals. The caller has
    /// made sure the number has no more than that.
    fn write_decimal(&self, f: &mut fmt::Formatter<'_>, decimals: u32) -> fmt::Result {
        let scaled = (&self.0 * BigRational::from_integer(ten_to(decimals))).to_integer();
        let digits = scaled.abs().to_string();
        let sign = if scaled.is_negative() { "-" } else { "" };
        let decimals = decimals as usize;
        if decimals == 0 {
            return write!(f, "{sign}{digits}");
        }
        let padded = format!("{digits:0>width$}", width = decimals + 1);
        let (whole, fraction) = padded.split_at(padded.len() - decimals);
        write!(f, "{sign}{whole}.{fraction}")
    }
}

/// Decimal notation where the number has one, fraction notation otherwise:
/// `12.5`, `-3`, `1/3`. With a precision (`{:.2}`) the number is rounded to
/// that many decimals, a tie away from zero.
impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(precision) = f.precision() {
            let decimals = u32::try_from(precision)
                .unwrap_or(u32::MAX)
                .min(MAX_PARSE_EXPONENT);
            // `decimals` is at most MAX_PARSE_EXPONENT, so it fits an i32.
            let rounded = self.round_to(decimals as i32, Rounding::HalfAwayFromZero);
            return rounded.write_decimal(f, decimals);
        }
        match self.decimal_places() {
            Some(decimals) => self.write_decimal(f, decimals),
            None => write!(f, "{}/{}", self.0.numer(), self.0.denom()),
        }
    }
}

impl fmt::Debug for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

/// Reads decimal notation (`12.50`, `-3`, `1e-7`) and fraction notation
/// (`1/3`), both exactly.
impl FromStr for Number {
    type Err = ParseNumberError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let fail = || ParseNumberError(text.to_string());
        let trimmed = text.trim();
        if let Some((numerator, denominator)) = trimmed.split_once('/') {
            let numerator: BigInt = numerator.trim().parse().map_err(|_| fail())?;
            let denominator: BigInt = denominator.trim().parse().map_err(|_| fail())?;
            if denominator.is_zero() {
                return Err(fail());
            }
            return Ok(Number(BigRational::new(numerator, denominator)));
        }
        let (mantissa, exponent) = match trimmed.split_once(['e', 'E']) {
            Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().map_err(|_| fail())?),
            None => (trimmed, 0),
        };
        let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
        let (negative, whole) = match whole.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, whole.strip_prefix('+').unwrap_or(whole)),
        };
        let all_digits = |part: &str| part.bytes().all(|b| b.is_ascii_digit());
        if (whole.is_empty() && fraction.is_empty()) || !all_digits(whole) || !all_digits(fraction)
        {
            return Err(fail());
        }
        let digits: BigInt = format!("{whole}{fraction}").parse().map_err(|_| fail())?;
        let fraction_len = i64::try_from(fraction.len()).map_err(|_| fail())?;
        let shift = exponent.checked_sub(fraction_len).ok_or_else(fail)?;
        let power = u32::try_from(shift.unsigned_abs())
            .ok()
            .filter(|power| *power <= MAX_PARSE_EXPONENT)
            .ok_or_else(fail)?;
        let value = if shift >= 0 {
            BigRational::from_integer(digits * ten_to(power))
        } else {
            BigRational::new(digits, ten_to(power))
        };
        Ok(Number(if negative { -value } else { value }))
    }
}

impl From<i64> for Number {
    fn from(value: i64) -> Self {
        Number(BigRational::from_integer(BigInt::from(value)))
    }
}

impl From<i32> for Number {
    fn from(value: i32) -> Self {
        Number::from(i64::from(value))
    }
}

impl From<u64> for Number {
    fn from(value: u64) -> Self {
        Number(BigRational::from_integer(BigInt::from(value)))
    }
}

impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Number {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl Add for &Number {
    type Output = Number;
    fn add(self, other: &Number) -> Number {
        Number(&self.0 + &other.0)
    }
}

impl Sub for &Number {
    type Output = Number;
    fn sub(self, other: &Number) -> Number {
        Number(&self.0 - &other.0)
    }
}

impl Mul for &Number {
    type Output = Number;
    fn mul(self, other: &Number) -> Number {
        Number(&self.0 * &other.0)
    }
}

impl Neg for &Number {
    type Output = Number;
    fn neg(self) -> Number {
        Number(-&self.0)
    }
}

/// Written as a JSON or YAML number through the nearest double, the ordinary
/// form RFC-055 keeps for the API and traces, so a whole number is written
/// with a fraction (`7.0`). The exact form for storage is in
/// [`crate::stored`].
impl Serialize for Number {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let double = self.to_f64().ok_or_else(|| {
            serde::ser::Error::custom(format!("{self} is too large to write as a number"))
        })?;
        serializer.serialize_f64(double)
    }
}

impl<'de> Deserialize<'de> for Number {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct NumberVisitor;

        impl de::Visitor<'_> for NumberVisitor {
            type Value = Number;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a number")
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Number, E> {
                Ok(Number::from(v))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Number, E> {
                Ok(Number::from(v))
            }

            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Number, E> {
                Number::from_f64(v).ok_or_else(|| E::custom("a number must be finite"))
            }
        }

        deserializer.deserialize_any(NumberVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(text: &str) -> Number {
        text.parse().expect("test literal should be a number")
    }

    #[test]
    fn a_third_times_three_is_one() {
        let third = num("1")
            .checked_div(&num("3"))
            .expect("divisor is not zero");
        assert_eq!(&third * &num("3"), num("1"));
    }

    #[test]
    fn decimal_notation_is_read_exactly() {
        assert_eq!(&num("0.1") + &num("0.2"), num("0.3"));
        assert_eq!(num("12.50"), num("12.5"));
        assert_eq!(num("-0.5"), num("-1/2"));
        assert_eq!(num("1e-3"), num("0.001"));
        assert_eq!(num("2.5E2"), num("250"));
        assert_eq!(num("+7"), num("7"));
        assert_eq!(num(".5"), num("0.5"));
    }

    #[test]
    fn text_that_is_no_number_is_refused() {
        for text in [
            "", "abc", "1.2.3", "1/0", "--1", "1e", ".", "1e999999", "0x10", "1 2",
        ] {
            assert!(text.parse::<Number>().is_err(), "{text:?} should not parse");
        }
    }

    #[test]
    fn a_double_is_read_as_its_shortest_decimal() {
        assert_eq!(Number::from_f64(0.1), Some(num("0.1")));
        assert_eq!(Number::from_f64(2.75), Some(num("2.75")));
        assert_eq!(Number::from_f64(-0.0), Some(Number::zero()));
        assert_eq!(Number::from_f64(1e21), Some(num("1e21")));
        assert_eq!(Number::from_f64(f64::NAN), None);
        assert_eq!(Number::from_f64(f64::INFINITY), None);
    }

    #[test]
    fn the_nearest_double_reads_back() {
        for text in ["0.1", "0.3693", "83.33", "-1234567.89", "1e-7"] {
            let double: f64 = text.parse().expect("test literal should be a double");
            assert_eq!(num(text).to_f64(), Some(double), "{text}");
        }
        let third = num("1/3").to_f64().expect("one third fits a double");
        assert_eq!(third, 1.0 / 3.0);
    }

    #[test]
    fn rounding_follows_the_mode() {
        let half = Rounding::HalfAwayFromZero;
        assert_eq!(num("2.5").round_to(0, half), num("3"));
        assert_eq!(num("-2.5").round_to(0, half), num("-3"));
        assert_eq!(num("2.449").round_to(1, half), num("2.4"));
        assert_eq!(num("1/3").round_to(2, half), num("0.33"));
        assert_eq!(num("1250").round_to(-2, half), num("1300"));
        assert_eq!(num("-1250").round_to(-2, half), num("-1300"));
        assert_eq!(num("2.01").round_to(0, Rounding::Ceil), num("3"));
        assert_eq!(num("-2.99").round_to(0, Rounding::Ceil), num("-2"));
        assert_eq!(num("2.99").round_to(0, Rounding::Floor), num("2"));
        assert_eq!(num("-2.01").round_to(0, Rounding::Floor), num("-3"));
        assert_eq!(num("149").round_to(-2, Rounding::Floor), num("100"));
    }

    #[test]
    fn decimal_places_tell_a_finite_decimal_from_a_fraction() {
        assert_eq!(num("5").decimal_places(), Some(0));
        assert_eq!(num("12.50").decimal_places(), Some(1));
        assert_eq!(num("0.125").decimal_places(), Some(3));
        assert_eq!(num("1/40").decimal_places(), Some(3));
        assert_eq!(num("1/3").decimal_places(), None);
        assert_eq!(num("5/12").decimal_places(), None);
    }

    #[test]
    fn display_writes_decimals_or_a_fraction() {
        assert_eq!(num("12.50").to_string(), "12.5");
        assert_eq!(num("-0.05").to_string(), "-0.05");
        assert_eq!(num("100").to_string(), "100");
        assert_eq!(num("1/3").to_string(), "1/3");
        assert_eq!(format!("{:.2}", num("1/3")), "0.33");
        assert_eq!(format!("{:.2}", num("2.005")), "2.01");
        assert_eq!(format!("{:.2}", num("-0.004")), "0.00");
        assert_eq!(format!("{:.0}", num("2.5")), "3");
        assert_eq!(format!("{:.2}", num("7")), "7.00");
    }

    #[test]
    fn fraction_notation_reads_back() {
        for text in ["1/3", "-22/7", "5", "0"] {
            assert_eq!(num(text).to_fraction_string(), text);
            assert_eq!(num(&num(text).to_fraction_string()), num(text));
        }
        assert_eq!(num("0.5").to_fraction_string(), "1/2");
    }

    #[test]
    fn whole_numbers_convert_to_i64() {
        assert_eq!(num("42").to_i64(), Some(42));
        assert_eq!(num("42.0").to_i64(), Some(42));
        assert_eq!(num("42.5").to_i64(), None);
        assert_eq!(num("1e30").to_i64(), None);
    }

    #[test]
    fn equal_values_are_equal_however_written() {
        use std::collections::HashSet;
        let set: HashSet<Number> = [num("1.0"), num("1.00"), num("2/2")].into_iter().collect();
        assert_eq!(set.len(), 1);
        assert!(num("0.3") < num("1/3"));
    }

    #[test]
    fn division_by_zero_gives_none() {
        assert_eq!(num("1").checked_div(&Number::zero()), None);
    }

    #[test]
    fn bits_grow_with_the_denominator() {
        assert!(num("1/3").bits() < num("1/1000000007").bits());
    }

    #[test]
    fn serializes_as_a_plain_number() {
        assert_eq!(
            serde_json::to_string(&num("12.5")).expect("serializable"),
            "12.5"
        );
        assert_eq!(
            serde_json::to_string(&num("7")).expect("serializable"),
            "7.0"
        );
        let back: Number = serde_json::from_str("0.1").expect("a number");
        assert_eq!(back, num("0.1"));
    }
}
