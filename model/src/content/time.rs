//! Points in time as authors and sources give them.
//!
//! Frontmatter often says only a day (`date: 2024-03-01`) and history gives an instant. Turning
//! a day into an instant needs a time zone the author never stated, so a [`When`] keeps the two
//! apart instead of guessing midnight somewhere.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use jiff::civil::Date;
use jiff::tz::Offset;
use serde::{Deserialize, Serialize};

pub use jiff::Timestamp;

/// A day, or an exact instant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum When {
    /// A calendar day with no time and no zone.
    Day(Date),
    /// An exact instant.
    Instant(Timestamp),
}

impl When {
    /// The calendar day: the day itself, or the instant's day in UTC.
    pub fn day(self) -> Date {
        match self {
            Self::Day(date) => date,
            Self::Instant(at) => Offset::UTC.to_datetime(at).date(),
        }
    }
}

impl Ord for When {
    /// By UTC day first; on the same day a bare day sorts before any instant, and instants by
    /// time. A total order, consistent with `Eq`, and the same on every machine.
    fn cmp(&self, other: &Self) -> Ordering {
        let key = |when: &Self| match *when {
            Self::Day(date) => (date, None),
            Self::Instant(at) => (when.day(), Some(at)),
        };
        key(self).cmp(&key(other))
    }
}

impl PartialOrd for When {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for When {
    /// `2024-03-01`, or RFC 3339 in UTC: `2024-03-01T09:30:00Z`.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Day(date) => write!(out, "{date}"),
            Self::Instant(at) => write!(out, "{at}"),
        }
    }
}

impl FromStr for When {
    type Err = WhenError;

    /// Exactly `YYYY-MM-DD` is a day; anything else must be an instant with its offset
    /// (`2024-03-01T09:30:00+01:00`). A day is never read out of a longer text.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() == "2024-03-01".len() {
            text.parse::<Date>().map(Self::Day).map_err(|source| WhenError::Day { source })
        } else {
            text.parse::<Timestamp>().map(Self::Instant).map_err(|source| WhenError::Instant { source })
        }
    }
}

/// Why text is no [`When`].
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum WhenError {
    /// Ten characters that are no calendar day.
    Day {
        /// What the calendar refused.
        source: jiff::Error,
    },
    /// Text that is no instant with an offset.
    Instant {
        /// What the parser refused.
        source: jiff::Error,
    },
}

impl fmt::Display for WhenError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Day { .. } => out.write_str("the text is no calendar day"),
            Self::Instant { .. } => out.write_str("the text is no time with an offset"),
        }
    }
}

impl std::error::Error for WhenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Day { source } | Self::Instant { source } => Some(source),
        }
    }
}

#[cfg(test)]
mod test {
    use jiff::civil::date;

    use super::{Timestamp, When};

    #[test]
    fn orders_days_against_instants() {
        let day = When::Day(date(2024, 3, 1));
        let morning = When::Instant("2024-03-01T09:00:00Z".parse::<Timestamp>().unwrap());
        let before = When::Instant("2024-02-29T23:59:00Z".parse::<Timestamp>().unwrap());
        let mut all = vec![morning, day, before];
        all.sort();
        assert_eq!(all, [before, day, morning]);
        assert_eq!(morning.to_string(), "2024-03-01T09:00:00Z");
        assert_eq!(day.to_string(), "2024-03-01");
    }

    #[test]
    fn parses_a_day_or_an_instant() {
        assert_eq!("2024-03-01".parse::<When>().unwrap(), When::Day(date(2024, 3, 1)));
        let instant = "2024-03-01T09:30:00+01:00".parse::<When>().unwrap();
        assert_eq!(instant.to_string(), "2024-03-01T08:30:00Z");
        assert!(matches!("2024-02-30".parse::<When>(), Err(super::WhenError::Day { .. })));
        assert!(matches!("2024-03-01 noon".parse::<When>(), Err(super::WhenError::Instant { .. })));
    }
}
