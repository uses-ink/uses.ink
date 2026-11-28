//! The object's logic.

use std::convert::Infallible;
use std::sync::{Mutex, PoisonError};

use edge_api::{Claimed, Claiming, Nonces};
use edge_object_nonce::Ledger;

use super::{Answer, Malformed, Wire};

/// A [`Ledger`] behind the wire. It is built when the object is, with the time then: that starts
/// the quiet period (the boot rule), so an object that was evicted and rebuilt refuses what was
/// created before it was.
#[derive(Debug)]
pub struct Serving {
    ledger: Mutex<Ledger>,
}

impl Serving {
    /// A serving ledger started at `boot` (unix seconds) for a window of `window` seconds.
    pub fn new(boot: u64, window: u64) -> Self {
        Self { ledger: Mutex::new(Ledger::new(boot, window)) }
    }

    /// Answers a claim: `body` is a [`Wire`] as JSON, the result an [`Answer`] as JSON.
    pub fn handle(&self, body: &[u8]) -> Result<Vec<u8>, Malformed> {
        let wire: Wire = serde_json::from_slice(body).map_err(|source| Malformed { source })?;
        let answer = self.answer(wire);
        serde_json::to_vec(&answer).map_err(|source| Malformed { source })
    }

    fn answer(&self, wire: Wire) -> Answer {
        self.ledger.lock().unwrap_or_else(PoisonError::into_inner).claim(wire.into()).into()
    }
}

impl Nonces for Serving {
    type Error = Infallible;

    fn claim(&self, claiming: Claiming) -> impl std::future::Future<Output = Result<Claimed, Infallible>> {
        std::future::ready(Ok(self.answer(claiming.into()).into()))
    }
}
