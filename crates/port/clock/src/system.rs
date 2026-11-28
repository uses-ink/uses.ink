//! The machine's clock and randomness.

use std::time::Duration;

use jiff::Timestamp;
use rand_core::{OsRng, RngCore};

use crate::{Clock, Draw, Slept};

/// The system clock, the tokio timer and the operating system's randomness.
#[derive(Clone, Copy, Debug, Default)]
pub struct System;

impl Clock for System {
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }

    async fn sleep(&self, time: Duration) -> Slept {
        tokio::time::sleep(time).await;
        Slept::Elapsed
    }
}

impl Draw for System {
    type Error = rand_core::Error;

    fn bytes(&self) -> Result<[u8; 16], rand_core::Error> {
        let mut bytes = [0_u8; 16];
        OsRng.try_fill_bytes(&mut bytes)?;
        Ok(bytes)
    }
}
