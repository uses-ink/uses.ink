//! What a delivery or a call may cost, as settings.

use std::time::Duration;

use jiff::SignedDuration;

/// The limits of one forge authority. Every one is a setting; none is a constant in the code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The most bytes of a webhook body it reads.
    pub body: usize,
    /// How long a delivery id is remembered, so a replay inside it is refused.
    pub window: SignedDuration,
    /// How many delivery ids it remembers; past it the oldest is forgotten first.
    pub remembered: usize,
    /// The most bytes of an API reply it reads.
    pub reply: usize,
    /// How long one API call may take in all.
    pub timeout: Duration,
}

impl Limits {
    /// A body of 5 MiB (GitHub sends up to 25 MiB for a push of many commits, which a site's
    /// build has no use for), ten minutes, ten thousand ids, a reply of 1 MiB, ten seconds.
    pub const DEFAULT: Self = Self {
        body: 5 * 1024 * 1024,
        window: SignedDuration::from_mins(10),
        remembered: 10_000,
        reply: 1024 * 1024,
        timeout: Duration::from_secs(10),
    };
}

impl Default for Limits {
    fn default() -> Self {
        Self::DEFAULT
    }
}
