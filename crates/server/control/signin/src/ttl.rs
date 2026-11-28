//! How long something lives.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use jiff::{SignedDuration, Timestamp};

/// A lifetime: at least one second and at most [`Ttl::LONGEST`]. Read from `90s`, `15m`, `12h`
/// or `7d`, so a flag or a settings file says what it means.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ttl(Duration);

/// A lifetime that is not usable.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TtlError {
    /// Less than a second.
    Short,
    /// More than [`Ttl::LONGEST`].
    Long {
        /// What was asked.
        asked: Duration,
    },
    /// Not a number followed by `s`, `m`, `h` or `d`.
    Written {
        /// What was written.
        text: String,
    },
}

impl fmt::Display for TtlError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Short => out.write_str("a lifetime is at least one second"),
            Self::Long { asked } => {
                write!(
                    out,
                    "a lifetime is at most {} days, not {} seconds",
                    Ttl::LONGEST.as_secs() / 86_400,
                    asked.as_secs()
                )
            }
            Self::Written { text } => {
                write!(out, "{text:?} is not a lifetime: write a number and s, m, h or d, as 15m")
            }
        }
    }
}

impl std::error::Error for TtlError {}

impl Ttl {
    /// The longest lifetime: 366 days.
    pub const LONGEST: Duration = Duration::from_hours(366 * 24);

    /// A lifetime of `duration`.
    pub fn new(duration: Duration) -> Result<Self, TtlError> {
        if duration < Duration::from_secs(1) {
            return Err(TtlError::Short);
        }
        if duration > Self::LONGEST {
            return Err(TtlError::Long { asked: duration });
        }
        Ok(Self(duration))
    }

    /// A lifetime of `seconds`, for a default that is known to be in range: a value outside the
    /// range is moved to the nearest end of it, never refused. Input from a person goes through
    /// [`Ttl::new`] or `FromStr`, which refuse it.
    pub const fn secs(seconds: u64) -> Self {
        let longest = Self::LONGEST.as_secs();
        let seconds = if seconds < 1 {
            1
        } else if seconds > longest {
            longest
        } else {
            seconds
        };
        Self(Duration::from_secs(seconds))
    }

    /// The lifetime as a duration.
    pub const fn duration(self) -> Duration {
        self.0
    }

    /// The time this lifetime ends when it starts at `from`.
    pub fn after(self, from: Timestamp) -> Result<Timestamp, jiff::Error> {
        from.checked_add(SignedDuration::try_from(self.0).unwrap_or(SignedDuration::MAX))
    }
}

impl FromStr for Ttl {
    type Err = TtlError;

    fn from_str(text: &str) -> Result<Self, TtlError> {
        let written = || TtlError::Written { text: text.chars().take(40).collect() };
        let split = text.len().checked_sub(1).ok_or_else(written)?;
        let (number, unit) = text.split_at_checked(split).ok_or_else(written)?;
        let scale = match unit {
            "s" => 1,
            "m" => 60,
            "h" => 3600,
            "d" => 86_400,
            _ => return Err(written()),
        };
        let count: u64 = number.parse().map_err(|_| written())?;
        Self::new(Duration::from_secs(count.checked_mul(scale).ok_or_else(written)?))
    }
}

impl fmt::Display for Ttl {
    /// The largest unit that holds it exactly.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let seconds = self.0.as_secs();
        match (seconds % 86_400, seconds % 3600, seconds % 60) {
            (0, ..) => write!(out, "{}d", seconds / 86_400),
            (_, 0, _) => write!(out, "{}h", seconds / 3600),
            (_, _, 0) => write!(out, "{}m", seconds / 60),
            _ => write!(out, "{seconds}s"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn reads_and_writes_the_four_units() {
        for text in ["90s", "15m", "12h", "7d"] {
            assert_eq!(text.parse::<Ttl>().unwrap().to_string(), text);
        }
        assert_eq!("60m".parse::<Ttl>().unwrap().to_string(), "1h");
    }

    #[test]
    fn refuses_what_is_no_lifetime() {
        assert_eq!("0s".parse::<Ttl>(), Err(TtlError::Short));
        assert!(matches!("400d".parse::<Ttl>(), Err(TtlError::Long { .. })));
        for text in ["", "m", "15", "1.5h", "-5m", "5w", "5 m"] {
            assert!(matches!(text.parse::<Ttl>(), Err(TtlError::Written { .. })), "{text}");
        }
    }
}
