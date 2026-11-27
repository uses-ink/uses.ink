//! What both halves of enrolling share: the key store, and the gate that closes the endpoints
//! while someone guesses codes or fills the queue of waiting keys (decision 0068, section 8).
//!
//! The API takes the unsigned ask (`POST /v1/enrol`) and the signed poll; the [`crate::Door`]
//! takes the person's approval. A guess at a code that does not exist, five times, closes both
//! for a while, as does having too many keys waiting. A code is 8 letters of 20, so a minute of
//! closing a time makes guessing one cost years, and the gate is memory: a restart opens it,
//! which gives a guesser nothing a patient one did not have.

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use control::keys::{Keys, Settings};
use control::signin::Ttl;
use jiff::Timestamp;

/// How the gate closes. Every number is the operator's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Closing {
    /// How many wrong codes close it. Default 5.
    pub guesses: u32,
    /// How many keys may wait for approval at once. Default 20.
    pub waiting: u64,
    /// How long it stays closed. Default one minute.
    pub closed: Ttl,
}

impl Default for Closing {
    fn default() -> Self {
        Self { guesses: 5, waiting: 20, closed: Ttl::secs(60) }
    }
}

/// The gate is closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Closed {
    /// How many seconds until it opens.
    pub seconds: u64,
}

impl fmt::Display for Closed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "enrolment is closed for {} more seconds", self.seconds)
    }
}

impl std::error::Error for Closed {}

#[derive(Debug, Default)]
struct Count {
    wrong: u32,
    until: Option<Timestamp>,
}

/// The gate: wrong guesses counted, and a time before which nothing is taken.
#[derive(Debug)]
pub struct Gate {
    rules: Closing,
    count: Mutex<Count>,
}

impl Gate {
    /// A gate that closes by `rules`.
    pub const fn new(rules: Closing) -> Self {
        Self { rules, count: Mutex::new(Count { wrong: 0, until: None }) }
    }

    /// How it closes.
    pub const fn rules(&self) -> &Closing {
        &self.rules
    }

    fn close(&self, count: &mut Count, now: Timestamp) {
        count.wrong = 0;
        count.until = self.rules.closed.after(now).ok();
    }

    /// Whether it is open at `now`.
    pub fn open(&self, now: Timestamp) -> Result<(), Closed> {
        let count = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        match count.until.filter(|until| *until > now) {
            Some(until) => {
                Err(Closed { seconds: u64::try_from(until.as_second() - now.as_second()).unwrap_or(1).max(1) })
            }
            None => Ok(()),
        }
    }

    /// Counts a wrong code at `now`; the one that reaches the limit closes the gate.
    pub fn miss(&self, now: Timestamp) {
        let mut count = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        count.wrong += 1;
        if count.wrong >= self.rules.guesses {
            self.close(&mut count, now);
        }
    }

    /// Whether `waiting` keys are too many to take another; if so it closes the gate.
    pub fn full(&self, waiting: u64, now: Timestamp) -> Result<(), Closed> {
        if waiting < self.rules.waiting {
            return Ok(());
        }
        let mut count = self.count.lock().unwrap_or_else(PoisonError::into_inner);
        self.close(&mut count, now);
        drop(count);
        self.open(now)
    }
}

/// The keys of the database and the one gate in front of enrolling, shared by the [`crate::Api`]
/// and the [`crate::Door`].
#[derive(Debug)]
pub struct Enrol<D> {
    keys: Keys<D>,
    gate: Arc<Gate>,
}

impl<D: Clone> Clone for Enrol<D> {
    fn clone(&self) -> Self {
        Self { keys: self.keys.clone(), gate: Arc::clone(&self.gate) }
    }
}

impl<D> Enrol<D> {
    /// The keys on `db` under `settings`, behind a gate that closes by `closing`.
    pub fn new(db: D, settings: Settings, closing: Closing) -> Self
    where
        D: db::Db,
    {
        Self { keys: Keys::new(db, settings), gate: Arc::new(Gate::new(closing)) }
    }

    /// The keys.
    pub const fn keys(&self) -> &Keys<D> {
        &self.keys
    }

    /// The gate.
    pub fn gate(&self) -> &Gate {
        &self.gate
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_second(seconds).unwrap()
    }

    #[test]
    fn five_wrong_codes_close_the_gate_for_a_minute() {
        let gate = Gate::new(Closing::default());
        for _ in 0..4 {
            gate.miss(at(100));
        }
        assert_eq!(gate.open(at(100)), Ok(()));
        gate.miss(at(100));
        assert_eq!(gate.open(at(130)), Err(Closed { seconds: 30 }));
        assert_eq!(gate.open(at(160)), Ok(()));
    }

    #[test]
    fn too_many_waiting_close_it_too() {
        let gate = Gate::new(Closing::default());
        assert_eq!(gate.full(19, at(100)), Ok(()));
        assert_eq!(gate.full(20, at(100)), Err(Closed { seconds: 60 }));
        assert!(gate.open(at(101)).is_err());
    }
}
