//! A wake-up that only writes down what it was asked.

use std::fmt;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::wake::{Nudge, Wake};

/// A [`Wake`] that keeps every nudge and can be told to refuse them.
#[derive(Debug, Default)]
pub struct Recorder {
    kept: Mutex<Vec<Nudge>>,
    refusing: AtomicBool,
}

/// What a [`Recorder`] says when it is told to refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refusal;

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the recorder was told to refuse the wake-up")
    }
}

impl std::error::Error for Refusal {}

impl Recorder {
    /// The nudges it was given and kept, in order.
    pub fn nudges(&self) -> Vec<Nudge> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// From now on refuses every nudge (or accepts them again).
    pub fn refuse(&self, refusing: bool) {
        self.refusing.store(refusing, Ordering::SeqCst);
    }
}

impl Wake for Recorder {
    type Error = Refusal;

    #[expect(clippy::unused_async_trait_impl, reason = "there is nothing to wait for")]
    async fn wake(&self, nudge: Nudge) -> Result<(), Refusal> {
        if self.refusing.load(Ordering::SeqCst) {
            return Err(Refusal);
        }
        self.kept.lock().unwrap_or_else(PoisonError::into_inner).push(nudge);
        Ok(())
    }
}
