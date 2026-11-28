//! A resolver that answers from a table, for tests and for an instance that has no DNS to ask.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt;

use crate::{Host, Resolver};

/// The fake's lookup did not get an answer, because it was told not to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unreachable;

impl fmt::Display for Unreachable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the fake resolver was told to fail")
    }
}

impl std::error::Error for Unreachable {}

/// A [`Resolver`] over records that were published to it. It asks no server.
#[derive(Debug, Default)]
pub struct Fake {
    records: RefCell<BTreeMap<Host, Vec<String>>>,
    down: Cell<bool>,
}

impl Fake {
    /// A resolver with no records.
    pub fn new() -> Self {
        Self::default()
    }

    /// Publishes `value` as a TXT record at `host`.
    pub fn publish(&self, host: &Host, value: impl Into<String>) {
        self.records.borrow_mut().entry(host.clone()).or_default().push(value.into());
    }

    /// Removes every record at `host`.
    pub fn withdraw(&self, host: &Host) {
        self.records.borrow_mut().remove(host);
    }

    /// Makes every lookup fail, or answer again.
    pub fn fail(&self, down: bool) {
        self.down.set(down);
    }
}

impl Resolver for Fake {
    type Error = Unreachable;

    fn txt(&self, host: &Host) -> impl Future<Output = Result<Vec<String>, Unreachable>> {
        std::future::ready(if self.down.get() {
            Err(Unreachable)
        } else {
            Ok(self.records.borrow().get(host).cloned().unwrap_or_default())
        })
    }
}

/// A server with no DNS to ask. Every lookup fails with [`NoResolver`], so a claim can be made and
/// listed and a proof says why it cannot be checked.
#[derive(Clone, Copy, Debug, Default)]
pub struct Offline;

/// The server has no resolver: the operator has not given it one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoResolver;

impl fmt::Display for NoResolver {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("this server has no DNS resolver, so it cannot check a proof record; the operator sets one")
    }
}

impl std::error::Error for NoResolver {}

impl Resolver for Offline {
    type Error = NoResolver;

    fn txt(&self, _: &Host) -> impl Future<Output = Result<Vec<String>, NoResolver>> {
        std::future::ready(Err(NoResolver))
    }
}
