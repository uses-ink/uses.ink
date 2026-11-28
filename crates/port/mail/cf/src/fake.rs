//! A transport that keeps what it is given.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use crate::{Request, Transport};

/// The fake provider is down: told to refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Down;

impl fmt::Display for Down {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the fake provider is down")
    }
}

impl std::error::Error for Down {}

/// Keeps every request in order, and refuses all of them while it is told to.
#[derive(Debug, Default)]
pub struct Fake {
    kept: Mutex<Vec<Request>>,
    down: AtomicBool,
}

impl Fake {
    /// A provider that is up and has taken nothing.
    pub const fn new() -> Self {
        Self { kept: Mutex::new(Vec::new()), down: AtomicBool::new(false) }
    }

    /// Everything taken so far.
    pub fn taken(&self) -> Vec<Request> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Makes every delivery fail (or stops doing so).
    pub fn down(&self, down: bool) {
        self.down.store(down, Ordering::SeqCst);
    }
}

impl Transport for Fake {
    type Error = Down;

    fn deliver(&self, request: &Request) -> impl std::future::Future<Output = Result<(), Down>> {
        let result = if self.down.load(Ordering::SeqCst) {
            Err(Down)
        } else {
            self.kept.lock().unwrap_or_else(PoisonError::into_inner).push(request.clone());
            Ok(())
        };
        std::future::ready(result)
    }
}
