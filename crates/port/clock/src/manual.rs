//! A clock a test moves.

use std::sync::Mutex;
use std::sync::PoisonError;
use std::time::Duration;

use jiff::{SignedDuration, Timestamp};

use crate::{Clock, Slept};

/// What a [`Manual`] clock holds.
#[derive(Debug)]
struct Hands {
    now: Timestamp,
    sleeps: Vec<Duration>,
    left: usize,
}

/// A clock that moves only when told to, and when something sleeps on it: a sleep returns at
/// once with the time moved on by what was asked, and is written down. After `rounds` sleeps it
/// says [`Slept::Stopped`], so a loop under test ends.
#[derive(Debug)]
pub struct Manual(Mutex<Hands>);

impl Manual {
    /// A clock at `start` that lets `rounds` sleeps pass.
    pub const fn new(start: Timestamp, rounds: usize) -> Self {
        Self(Mutex::new(Hands { now: start, sleeps: Vec::new(), left: rounds }))
    }

    fn hands(&self) -> std::sync::MutexGuard<'_, Hands> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Moves the time on.
    pub fn advance(&self, time: Duration) {
        let mut hands = self.hands();
        hands.now =
            hands.now.checked_add(SignedDuration::try_from(time).unwrap_or(SignedDuration::MAX)).unwrap_or(hands.now);
    }

    /// Every wait asked of the clock, in order.
    pub fn sleeps(&self) -> Vec<Duration> {
        self.hands().sleeps.clone()
    }
}

impl Clock for Manual {
    fn now(&self) -> Timestamp {
        self.hands().now
    }

    fn sleep(&self, time: Duration) -> impl std::future::Future<Output = Slept> {
        std::future::ready(self.rest(time))
    }
}

impl Manual {
    fn rest(&self, time: Duration) -> Slept {
        let mut hands = self.hands();
        hands.sleeps.push(time);
        if hands.left == 0 {
            return Slept::Stopped;
        }
        hands.left -= 1;
        drop(hands);
        self.advance(time);
        Slept::Elapsed
    }
}
