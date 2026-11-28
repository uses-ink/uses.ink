//! The port that reads DNS.

use std::future::Future;

use crate::Host;

/// Reads the TXT records of a host. Async and not `Send`, like the other ports of the control
/// plane, so the same code runs where a request holds its connection and where a Worker does.
///
/// An answer is every record the host has, as text, in no order. A host with no records is an
/// empty list, not an error: an error is a lookup that did not get an answer, which is not the
/// same fact and is worded differently.
pub trait Resolver {
    /// What a lookup fails with.
    type Error: std::error::Error + 'static;

    /// The TXT records at `host`.
    fn txt(&self, host: &Host) -> impl Future<Output = Result<Vec<String>, Self::Error>>;
}
