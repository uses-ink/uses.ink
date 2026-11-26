//! Points in time as authors and sources give them.
//!
//! Frontmatter often says only a day (`date: 2024-03-01`) and history gives an instant. Turning
//! a day into an instant needs a time zone the author never stated, so a [`When`] keeps the two
//! apart instead of guessing midnight somewhere.

use std::cmp::Ordering;
use std::fmt;

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
}
