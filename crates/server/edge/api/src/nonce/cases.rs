//! The conformance suite of the nonce store: what any [`Nonces`] must do, as async cases a native
//! test and a Worker can both drive.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::missing_panics_doc,
    reason = "a conformance case stops at the first surprise"
)]

use sig::{KeyId, Nonce};

use super::{Claimed, Claiming, Nonces};

/// The cases, each with the key its claims use: one case's claims never touch another's, so a store
/// that outlives a case (a Durable Object) is not in the way of the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Case {
    /// The first claim wins and a second is a replay.
    Replays,
    /// Another key may use the same nonce.
    Splits,
    /// A claim ends at its `until`.
    Ends,
    /// A key holds at most `most` nonces, and room comes back as they end.
    Bounds,
    /// A replay at the bound is a replay, not a flood.
    Orders,
}

impl Case {
    /// Every case, in the order the suite runs them.
    pub const ALL: &'static [Self] = &[Self::Replays, Self::Splits, Self::Ends, Self::Bounds, Self::Orders];

    /// The window the cases assume the store was built for, in seconds.
    pub const WINDOW: u64 = 60;

    /// Its name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Replays => "claims_once_and_refuses_a_replay",
            Self::Splits => "keeps_each_key_to_itself",
            Self::Ends => "forgets_a_nonce_when_its_window_ends",
            Self::Bounds => "bounds_a_key_and_frees_room_as_nonces_end",
            Self::Orders => "says_replay_before_full",
        }
    }

    /// The case named `name`, if there is one.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|case| case.name() == name)
    }

    /// The key the case claims under: its place in [`Case::ALL`] in every byte, then `salt`.
    fn key(self, salt: u8) -> KeyId {
        let place = Self::ALL.iter().position(|case| *case == self).unwrap_or(0);
        let mut bytes = [u8::try_from(place).unwrap_or(0); 16];
        bytes[15] = salt;
        KeyId::new(bytes)
    }

    /// A claim of the case's key `salt` for nonce `n` (`whose`), claimed at `now` and ending `WINDOW` later.
    fn claiming(self, whose: (u8, u8), now: u64, most: usize) -> Claiming {
        let (salt, n) = whose;
        Claiming { key: self.key(salt), nonce: Nonce::new([n; 16]), until: now + Self::WINDOW, now, most }
    }

    /// Runs the case on `nonces`. `base` is a time after the store's quiet period (a store built at
    /// time 0 takes any positive one); the case uses `base` and the seconds after it. It panics at
    /// the first surprise.
    pub async fn run<N: Nonces>(self, nonces: &N, base: u64) {
        match self {
            Self::Replays => {
                assert_eq!(nonces.claim(self.claiming((0, 1), base, 4)).await.unwrap(), Claimed::Fresh);
                assert_eq!(nonces.claim(self.claiming((0, 1), base + 1, 4)).await.unwrap(), Claimed::Replay);
                assert_eq!(nonces.claim(self.claiming((0, 2), base + 1, 4)).await.unwrap(), Claimed::Fresh);
            }
            Self::Splits => {
                assert_eq!(nonces.claim(self.claiming((0, 1), base, 4)).await.unwrap(), Claimed::Fresh);
                assert_eq!(nonces.claim(self.claiming((1, 1), base, 4)).await.unwrap(), Claimed::Fresh);
                assert_eq!(nonces.claim(self.claiming((1, 1), base + 1, 4)).await.unwrap(), Claimed::Replay);
            }
            Self::Ends => {
                assert_eq!(nonces.claim(self.claiming((0, 1), base, 4)).await.unwrap(), Claimed::Fresh);
                let last = base + Self::WINDOW - 1;
                assert_eq!(nonces.claim(self.claiming((0, 1), last, 4)).await.unwrap(), Claimed::Replay);
                let ended = base + Self::WINDOW;
                assert_eq!(nonces.claim(self.claiming((0, 1), ended, 4)).await.unwrap(), Claimed::Fresh);
            }
            Self::Bounds => {
                assert_eq!(nonces.claim(self.claiming((0, 1), base, 2)).await.unwrap(), Claimed::Fresh);
                assert_eq!(nonces.claim(self.claiming((0, 2), base + 1, 2)).await.unwrap(), Claimed::Fresh);
                assert_eq!(nonces.claim(self.claiming((0, 3), base + 2, 2)).await.unwrap(), Claimed::Full);
                // the first ends at base + WINDOW: its room is free, the second's is not yet
                let freed = base + Self::WINDOW;
                assert_eq!(nonces.claim(self.claiming((0, 3), freed, 2)).await.unwrap(), Claimed::Fresh);
                assert_eq!(nonces.claim(self.claiming((0, 4), freed, 2)).await.unwrap(), Claimed::Full);
                assert_eq!(nonces.claim(self.claiming((1, 1), freed, 2)).await.unwrap(), Claimed::Fresh);
            }
            Self::Orders => {
                assert_eq!(nonces.claim(self.claiming((0, 1), base, 1)).await.unwrap(), Claimed::Fresh);
                assert_eq!(nonces.claim(self.claiming((0, 1), base + 1, 1)).await.unwrap(), Claimed::Replay);
                assert_eq!(nonces.claim(self.claiming((0, 2), base + 1, 1)).await.unwrap(), Claimed::Full);
            }
        }
    }
}
