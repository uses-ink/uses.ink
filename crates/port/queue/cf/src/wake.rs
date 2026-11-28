//! The wake-up: what is sent, and what sends it.

use std::convert::Infallible;
use std::time::Duration;

/// A request to run a consumer, after a delay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Nudge {
    seconds: u32,
}

impl Nudge {
    /// The longest delay Cloudflare Queues takes on a message: 12 hours. A later visibility is
    /// nudged at this delay, and the consumer finds nothing then; the Cron trigger covers the rest.
    pub const MOST: u32 = 43_200;

    /// A nudge `delay` from now, rounded up to whole seconds and held to [`Nudge::MOST`].
    pub fn after(delay: Duration) -> Self {
        let seconds = delay.as_secs().saturating_add(u64::from(delay.subsec_nanos() > 0));
        Self { seconds: u32::try_from(seconds).map_or(Self::MOST, |seconds| seconds.min(Self::MOST)) }
    }

    /// The delay in whole seconds.
    pub const fn seconds(&self) -> u32 {
        self.seconds
    }
}

/// Something that sends a [`Nudge`]. Futures are not required to be `Send`.
#[expect(async_fn_in_trait, reason = "a Queues binding's futures are not Send on Workers")]
pub trait Wake {
    /// What a failed wake-up says.
    type Error: std::error::Error + 'static;

    /// Asks a consumer to run after the nudge's delay.
    async fn wake(&self, nudge: Nudge) -> Result<(), Self::Error>;
}

/// No wake-up: the consumer runs on a Cron trigger alone.
#[derive(Clone, Copy, Debug, Default)]
pub struct Silent;

impl Wake for Silent {
    type Error = Infallible;

    #[expect(clippy::unused_async_trait_impl, reason = "there is nothing to wait for")]
    async fn wake(&self, _: Nudge) -> Result<(), Infallible> {
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_delay_rounds_up_and_stops_at_what_queues_takes() {
        assert_eq!(Nudge::after(Duration::ZERO).seconds(), 0);
        assert_eq!(Nudge::after(Duration::from_millis(1)).seconds(), 1);
        assert_eq!(Nudge::after(Duration::from_secs(90)).seconds(), 90);
        assert_eq!(Nudge::after(Duration::from_hours(24)).seconds(), Nudge::MOST);
        assert_eq!(Nudge::after(Duration::MAX).seconds(), Nudge::MOST);
    }
}
