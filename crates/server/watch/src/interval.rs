//! How often to poll.

use std::fmt;
use std::time::Duration;

/// How often a source is polled, and how far that backs off.
///
/// A source that has not moved is asked half as often after each look, down to `max`; one that
/// moves is asked every `base` again. `base` has a floor, so no setting is a busy loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interval {
    base: Duration,
    max: Duration,
    lookout: Duration,
}

/// An interval that would hammer a remote or make no sense.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum IntervalError {
    /// The base is under the floor.
    Fast {
        /// What was asked.
        base: Duration,
        /// The floor.
        floor: Duration,
    },
    /// The longest wait is under the base.
    Backwards {
        /// The base.
        base: Duration,
        /// The longest wait.
        max: Duration,
    },
    /// The lookout is under its floor, so the loop would spin.
    Lookout {
        /// What was asked.
        lookout: Duration,
        /// The floor.
        floor: Duration,
    },
}

impl fmt::Display for IntervalError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fast { base, floor } => {
                write!(out, "polling every {}s is under the floor of {}s", base.as_secs(), floor.as_secs())
            }
            Self::Backwards { base, max } => {
                write!(out, "the longest wait of {}s is under the base of {}s", max.as_secs(), base.as_secs())
            }
            Self::Lookout { lookout, floor } => {
                write!(
                    out,
                    "looking out for new sites every {}ms is under the floor of {}ms",
                    lookout.as_millis(),
                    floor.as_millis()
                )
            }
        }
    }
}

impl std::error::Error for IntervalError {}

impl Interval {
    /// The shortest base: ten seconds.
    pub const FLOOR: Duration = Duration::from_secs(10);

    /// The shortest lookout: one second.
    pub const LOOKOUT_FLOOR: Duration = Duration::from_secs(1);

    /// Poll every `base`, backing off to `max`, looking for new sites every five seconds.
    pub fn new(base: Duration, max: Duration) -> Result<Self, IntervalError> {
        if base < Self::FLOOR {
            return Err(IntervalError::Fast { base, floor: Self::FLOOR });
        }
        if max < base {
            return Err(IntervalError::Backwards { base, max });
        }
        Ok(Self { base, max, lookout: Duration::from_secs(5) })
    }

    /// The same intervals, looking for sites that were added every `lookout`: a site added after
    /// the loop went to rest is polled for the first time within that long, however far off the
    /// next poll of the others is.
    pub fn with_lookout(self, lookout: Duration) -> Result<Self, IntervalError> {
        if lookout < Self::LOOKOUT_FLOOR {
            return Err(IntervalError::Lookout { lookout, floor: Self::LOOKOUT_FLOOR });
        }
        Ok(Self { lookout, ..self })
    }

    /// How long the loop rests at most, so a new site is noticed.
    pub const fn lookout(&self) -> Duration {
        self.lookout
    }

    /// The wait after a source moved, and the first.
    pub const fn base(&self) -> Duration {
        self.base
    }

    /// The longest wait.
    pub const fn max(&self) -> Duration {
        self.max
    }

    /// The wait after `wait` when nothing changed or the remote did not answer.
    pub fn backed(&self, wait: Duration) -> Duration {
        wait.saturating_mul(2).min(self.max)
    }
}

impl Default for Interval {
    /// Every 10 minutes, backing off to 1 hour (architecture section 12).
    fn default() -> Self {
        Self { base: Duration::from_secs(600), max: Duration::from_secs(3600), lookout: Duration::from_secs(5) }
    }
}
