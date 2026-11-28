//! The nonce store's state machine (decision 0135), without a Worker in it, so it is tested
//! natively and a Durable Object is a thin shell around it.
//!
//! A [`Ledger`] remembers `(key, nonce)` pairs until the end of their window, at most `most` per
//! key. It is built with the time it starts and the window, and from then it refuses any request
//! that was created before `boot + window` with [`Verdict::Quiet`]: a ledger that was rebuilt
//! (evicted, moved, its storage lost) cannot know what a request captured before it was built
//! already used. It reads no clock; every call says what time it is.

use std::collections::HashMap;

/// Sixteen bytes: a key's id, or a nonce.
pub type Bytes = [u8; 16];

/// One claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ask {
    /// The key that signed.
    pub key: Bytes,
    /// The request's nonce.
    pub nonce: Bytes,
    /// When the claim ends (unix seconds): the request's `created` plus the window.
    pub until: u64,
    /// When it is claimed (unix seconds).
    pub now: u64,
    /// How many nonces one key may have at once.
    pub most: usize,
}

/// What a claim found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// New: now remembered.
    Fresh,
    /// Seen before inside its window.
    Replay,
    /// The key has as many nonces as are remembered at once.
    Full,
    /// The ledger is new and the request is older than its quiet period.
    Quiet {
        /// Seconds until the quiet period ends, at least one.
        seconds: u64,
    },
}

/// The nonces seen, per key, each until the end of its window.
#[derive(Debug)]
pub struct Ledger {
    quiet_until: u64,
    window: u64,
    seen: HashMap<Bytes, HashMap<Bytes, u64>>,
}

impl Ledger {
    /// An empty ledger started at `boot` (unix seconds) for requests with a window of `window`.
    pub fn new(boot: u64, window: u64) -> Self {
        Self { quiet_until: boot.saturating_add(window), window, seen: HashMap::new() }
    }

    /// The time the quiet period ends.
    pub const fn quiet_until(&self) -> u64 {
        self.quiet_until
    }

    /// How many nonces are remembered, over every key.
    pub fn len(&self) -> usize {
        self.seen.values().map(HashMap::len).sum()
    }

    /// Whether nothing is remembered.
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }

    /// Remembers the nonce of `ask` until its end, or says why not. What ended before `ask.now` is
    /// forgotten first.
    pub fn claim(&mut self, ask: Ask) -> Verdict {
        let created = ask.until.saturating_sub(self.window);
        if created < self.quiet_until {
            return Verdict::Quiet { seconds: self.quiet_until.saturating_sub(ask.now).max(1) };
        }
        self.seen.retain(|_, nonces| {
            nonces.retain(|_, end| *end > ask.now);
            !nonces.is_empty()
        });
        let nonces = self.seen.entry(ask.key).or_default();
        if nonces.contains_key(&ask.nonce) {
            return Verdict::Replay;
        }
        if nonces.len() >= ask.most {
            return Verdict::Full;
        }
        nonces.insert(ask.nonce, ask.until);
        Verdict::Fresh
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn ask(nonce: u8, created: u64, now: u64) -> Ask {
        Ask { key: [1; 16], nonce: [nonce; 16], until: created + 60, now, most: 2 }
    }

    #[test]
    fn claims_once_bounds_a_key_and_forgets_at_the_end() {
        let mut ledger = Ledger::new(1_000, 60);
        assert!(ledger.is_empty());
        assert_eq!(ledger.claim(ask(1, 1_100, 1_100)), Verdict::Fresh);
        assert_eq!(ledger.claim(ask(1, 1_100, 1_101)), Verdict::Replay);
        assert_eq!(ledger.claim(ask(2, 1_100, 1_101)), Verdict::Fresh);
        assert_eq!(ledger.claim(ask(3, 1_100, 1_102)), Verdict::Full);
        assert_eq!(ledger.len(), 2);
        assert_eq!(ledger.claim(ask(1, 1_200, 1_160)), Verdict::Fresh);
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn refuses_what_was_created_before_its_quiet_period_ended() {
        let mut ledger = Ledger::new(1_000, 60);
        assert_eq!(ledger.quiet_until(), 1_060);
        assert_eq!(ledger.claim(ask(1, 1_010, 1_020)), Verdict::Quiet { seconds: 40 });
        assert_eq!(ledger.claim(ask(1, 1_059, 1_070)), Verdict::Quiet { seconds: 1 });
        assert!(ledger.is_empty());
        assert_eq!(ledger.claim(ask(1, 1_060, 1_070)), Verdict::Fresh);
    }
}
