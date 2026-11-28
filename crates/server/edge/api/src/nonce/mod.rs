//! The nonce store (decision 0135): [`Nonces`] is the port, [`Remembered`] the memory store, and
//! [`Case`] lists the suite every store passes.
//!
//! A signed request's `(key, nonce)` pair is claimed once the signature verifies, and is kept
//! until the end of its window. A store may also refuse for a quiet period after it was built
//! ([`Claimed::Quiet`]): a store that lost its memory must not take a request captured before.

use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Mutex, PoisonError};

use sig::{KeyId, Nonce};

mod cases;

pub use cases::Case;

/// What claiming a nonce found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Claimed {
    /// New: now remembered.
    Fresh,
    /// Seen before inside its window.
    Replay,
    /// The key has as many nonces as are remembered at once.
    Full,
    /// The store is new and the request is older than its quiet period: try again.
    Quiet {
        /// Seconds until the quiet period ends.
        seconds: u64,
    },
}

/// One claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Claiming {
    /// The key that signed.
    pub key: KeyId,
    /// The request's nonce.
    pub nonce: Nonce,
    /// When the claim ends (unix seconds): the request's `created` plus the window.
    pub until: u64,
    /// When it is claimed (unix seconds): what ended before it is forgotten first.
    pub now: u64,
    /// How many nonces one key may have at once.
    pub most: usize,
}

/// Remembers nonces. Futures are not required to be `Send`.
#[expect(async_fn_in_trait, reason = "a Durable Object stub's futures are not Send")]
pub trait Nonces {
    /// The store's own error: it could not be reached.
    type Error: std::error::Error + 'static;

    /// Remembers the nonce of `claiming` until its end, or says why not.
    async fn claim(&self, claiming: Claiming) -> Result<Claimed, Self::Error>;
}

/// The nonces seen, per key, each until the end of its window. Memory only: a restart loses them,
/// and the boot barrier of [`crate::Signatures`] closes that.
#[derive(Debug, Default)]
pub struct Remembered {
    seen: Mutex<HashMap<KeyId, HashMap<Nonce, u64>>>,
}

impl Remembered {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    fn remember(&self, claiming: Claiming) -> Claimed {
        let Claiming { key, nonce, until, now, most } = claiming;
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        seen.retain(|_, nonces| {
            nonces.retain(|_, end| *end > now);
            !nonces.is_empty()
        });
        let nonces = seen.entry(key).or_default();
        if nonces.contains_key(&nonce) {
            return Claimed::Replay;
        }
        if nonces.len() >= most {
            return Claimed::Full;
        }
        nonces.insert(nonce, until);
        Claimed::Fresh
    }
}

impl Nonces for Remembered {
    type Error = Infallible;

    fn claim(&self, claiming: Claiming) -> impl std::future::Future<Output = Result<Claimed, Infallible>> {
        std::future::ready(Ok(self.remember(claiming)))
    }
}
