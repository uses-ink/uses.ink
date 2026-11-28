//! Delivery ids and the window that refuses a replay.

use std::collections::{BTreeSet, VecDeque};
use std::fmt;
use std::str::FromStr;
use std::sync::{Mutex, PoisonError};

use jiff::{SignedDuration, Timestamp};

use crate::NameError;

/// The id a forge gave one delivery: 1 to 128 visible ASCII characters.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Delivery(String);

impl Delivery {
    /// The longest an id is.
    pub const LIMIT: usize = 128;
}

impl fmt::Display for Delivery {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl FromStr for Delivery {
    type Err = NameError;

    fn from_str(text: &str) -> Result<Self, NameError> {
        if text.is_empty() {
            return Err(NameError::Empty);
        }
        if text.len() > Self::LIMIT {
            return Err(NameError::Long { found: text.len(), limit: Self::LIMIT });
        }
        match text.bytes().position(|byte| !byte.is_ascii_graphic()) {
            Some(at) => Err(NameError::Character { at }),
            None => Ok(Self(text.to_owned())),
        }
    }
}

/// A delivery id that was seen already inside the window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replayed {
    /// The id.
    pub delivery: Delivery,
}

#[derive(Debug, Default)]
struct Seen {
    order: VecDeque<(Timestamp, Delivery)>,
    set: BTreeSet<Delivery>,
}

/// The ids seen lately. A signed body proves who sent it and not when, and the delivery id is not
/// under the signature, so this is a bounded memory and not a proof: it refuses the same id twice
/// inside the window, and the build queue's dedupe on site and commit is what makes a replay
/// with a fresh id harmless.
#[derive(Debug)]
pub struct Replay {
    window: SignedDuration,
    cap: usize,
    seen: Mutex<Seen>,
}

impl Replay {
    /// A memory of `cap` ids that each last `window`.
    pub fn new(window: SignedDuration, cap: usize) -> Self {
        Self { window, cap, seen: Mutex::new(Seen::default()) }
    }

    /// Takes `delivery` as seen at `now`, or refuses it when it was seen inside the window. Only
    /// a delivery whose signature held is passed here, so a stranger cannot fill the memory.
    pub fn admit(&self, delivery: &Delivery, now: Timestamp) -> Result<(), Replayed> {
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        while let Some((at, old)) = seen.order.front() {
            let ended = now.duration_since(*at) >= self.window;
            if !ended && seen.order.len() <= self.cap {
                break;
            }
            let old = old.clone();
            seen.order.pop_front();
            seen.set.remove(&old);
        }
        if !seen.set.insert(delivery.clone()) {
            return Err(Replayed { delivery: delivery.clone() });
        }
        seen.order.push_back((now, delivery.clone()));
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn id(text: &str) -> Delivery {
        text.parse().unwrap()
    }

    #[test]
    fn an_id_repeats_only_after_the_window() {
        let replay = Replay::new(SignedDuration::from_secs(60), 10);
        let start = Timestamp::from_second(1_800_000_000).unwrap();
        replay.admit(&id("a"), start).unwrap();
        assert_eq!(replay.admit(&id("a"), start + SignedDuration::from_secs(59)), Err(Replayed { delivery: id("a") }));
        replay.admit(&id("a"), start + SignedDuration::from_secs(61)).unwrap();
    }

    #[test]
    fn the_memory_is_bounded_and_forgets_the_oldest() {
        let replay = Replay::new(SignedDuration::from_hours(1), 2);
        let start = Timestamp::from_second(1_800_000_000).unwrap();
        for name in ["a", "b", "c"] {
            replay.admit(&id(name), start).unwrap();
        }
        replay.admit(&id("a"), start).unwrap();
        assert!(replay.admit(&id("c"), start).is_err());
    }

    #[test]
    fn an_id_is_visible_ascii() {
        assert!("".parse::<Delivery>().is_err());
        assert_eq!("a b".parse::<Delivery>(), Err(NameError::Character { at: 1 }));
    }
}
