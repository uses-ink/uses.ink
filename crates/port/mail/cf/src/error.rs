//! What the adapter fails with.

use std::fmt;

use mail::Address;

/// The transport refused or failed. The recipient is named, the body never.
#[derive(Debug)]
pub struct Error<E> {
    /// Who the message was for.
    pub to: Address,
    /// What the transport said.
    pub source: E,
}

impl<E> fmt::Display for Error<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "the mail provider did not take the message to {}", self.to)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
