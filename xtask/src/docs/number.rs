//! A decision's number as the docs write it: four digits, from 0001.

use std::error::Error;
use std::fmt;

use serde::Deserialize;

/// A decision number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct Number(u16);

/// Why a text is no decision number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// Not four ASCII digits.
    Digits(String),
    /// 0000, which no decision has.
    Zero,
}

impl Number {
    /// How many digits it is written with.
    pub(crate) const WIDTH: usize = 4;

    /// The first decision.
    pub(crate) const FIRST: Self = Self(1);

    /// Reads exactly four digits, not zero.
    pub(crate) fn parse(text: &str) -> Result<Self, Refusal> {
        let digits = text.len() == Self::WIDTH && text.bytes().all(|byte| byte.is_ascii_digit());
        match text.parse::<u16>() {
            Ok(0) if digits => Err(Refusal::Zero),
            Ok(number) if digits => Ok(Self(number)),
            _ => Err(Refusal::Digits(text.to_owned())),
        }
    }

    pub(crate) const fn get(self) -> u16 {
        self.0
    }

    /// Every number from this one to `last`, both included.
    pub(crate) fn through(self, last: Self) -> impl Iterator<Item = Self> {
        (self.0..=last.0).map(Self)
    }

    /// The number `value`, for the tests.
    #[cfg(test)]
    pub(crate) const fn sample(value: u16) -> Self {
        Self(value)
    }
}

impl fmt::Display for Number {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{:04}", self.0)
    }
}

impl TryFrom<String> for Number {
    type Error = Refusal;

    fn try_from(text: String) -> Result<Self, Refusal> {
        Self::parse(&text)
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Digits(given) => write!(out, "`{given}` is not four digits"),
            Self::Zero => write!(out, "0000 is not a decision number"),
        }
    }
}

impl Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_digits_are_a_number() {
        assert_eq!(Number::parse("0042"), Ok(Number::sample(42)));
        assert_eq!(Number::sample(7).to_string(), "0007");
    }

    #[test]
    fn anything_else_is_refused() {
        assert_eq!(Number::parse("42"), Err(Refusal::Digits("42".to_owned())));
        assert_eq!(Number::parse("+042"), Err(Refusal::Digits("+042".to_owned())));
        assert_eq!(Number::parse("00x2"), Err(Refusal::Digits("00x2".to_owned())));
        assert_eq!(Number::parse("0000"), Err(Refusal::Zero));
    }

    #[test]
    fn a_range_runs_through_both_ends() {
        let found: Vec<u16> = Number::sample(3).through(Number::sample(5)).map(Number::get).collect();
        assert_eq!(found, [3, 4, 5]);
    }
}
