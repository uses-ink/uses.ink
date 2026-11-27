//! Times as the cache files hold them, and spans as a person says them.

use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// A time as whole seconds since the epoch: what the cache files hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Moment(pub(crate) u64);

impl Moment {
    /// A moment of the system clock.
    pub(crate) fn now() -> Self {
        Self::of(SystemTime::now())
    }

    /// A moment of the system clock.
    pub(crate) fn of(time: SystemTime) -> Self {
        Self(time.duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs()))
    }

    /// How long before `now` this was.
    pub(crate) const fn age(self, now: Self) -> Duration {
        Duration::from_secs(now.0.saturating_sub(self.0))
    }
}

/// A span as a person says it: `40 s`, `3 min`, `5 h`, `2 d`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ago(pub(crate) Duration);

impl fmt::Display for Ago {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.0.as_secs();
        match secs {
            0..=59 => write!(out, "{secs} s"),
            60..=3599 => write!(out, "{} min", secs / 60),
            3600..=86_399 => write!(out, "{} h", secs / 3600),
            _ => write!(out, "{} d", secs / 86_400),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_read_as_a_person_says_them() {
        let say = |secs| Ago(Duration::from_secs(secs)).to_string();
        assert_eq!(
            (say(5), say(180), say(7200), say(200_000)),
            ("5 s".to_owned(), "3 min".to_owned(), "2 h".to_owned(), "2 d".to_owned())
        );
    }
}
