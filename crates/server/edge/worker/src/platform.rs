//! The clock and the randomness of a Worker.

use std::time::Duration;

use clock::{Clock, Draw, Slept, System};
use jiff::Timestamp;

/// The Worker's clock: `Date.now()` for the time, `setTimeout` for a wait and `crypto.getRandomValues`
/// for ids. [`clock::System`] reads the operating system's clock and the tokio timer, and a Worker
/// has neither.
#[derive(Clone, Copy, Debug, Default)]
pub struct Platform;

impl Clock for Platform {
    fn now(&self) -> Timestamp {
        // `Date.now()` is a count of milliseconds a `Timestamp` holds until the year 275760
        let millis = i64::try_from(worker::Date::now().as_millis()).unwrap_or(i64::MAX);
        Timestamp::from_millisecond(millis).unwrap_or(Timestamp::MAX)
    }

    async fn sleep(&self, time: Duration) -> Slept {
        worker::Delay::from(time).await;
        Slept::Elapsed
    }
}

impl Draw for Platform {
    type Error = <System as Draw>::Error;

    fn bytes(&self) -> Result<[u8; 16], Self::Error> {
        System.bytes()
    }
}
