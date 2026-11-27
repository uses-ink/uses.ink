//! Time and randomness as ports (architecture section 2: no use case reads a clock or draws a
//! random number for itself).
//!
//! A [`Clock`] says what time it is and waits; a [`Draw`] gives 16 random bytes, the size of
//! every id. [`System`] is the adapter that reads the machine's clock, sleeps on the tokio timer
//! and asks the operating system for randomness. [`Manual`] and [`Counter`] are the stand-ins a
//! test uses: time moves only when it is told to or when something sleeps, and ids count up, so
//! the same calls make the same rows and no test waits.

mod draw;
mod manual;
mod system;

pub use draw::{Counter, Draw};
pub use manual::Manual;
pub use system::System;

use std::time::Duration;

use jiff::Timestamp;

/// How a wait ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Slept {
    /// The time passed.
    Elapsed,
    /// The clock has nothing more to give: a loop that waits on it ends. The system clock never
    /// says this; a test clock does, so a loop is run for a fixed number of rounds.
    Stopped,
}

/// What time it is, and a wait. Futures are not required to be `Send`.
#[expect(async_fn_in_trait, reason = "a Durable Object's timer is not Send on Workers")]
pub trait Clock {
    /// The time now.
    fn now(&self) -> Timestamp;

    /// Waits for `time` to pass without holding a thread.
    async fn sleep(&self, time: Duration) -> Slept;
}

impl<C: Clock + ?Sized> Clock for &C {
    fn now(&self) -> Timestamp {
        (**self).now()
    }

    async fn sleep(&self, time: Duration) -> Slept {
        (**self).sleep(time).await
    }
}
