//! What sending mail over SMTP fails with.

use std::fmt;

/// A failure to set up the connection or to send. Each variant keeps the library's error as its
/// source, and none holds the password.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// An address the library does not accept.
    Address(lettre::address::AddressError),
    /// The message could not be built.
    Message(lettre::error::Error),
    /// The connection could not be set up, or the server refused the message.
    Transport(lettre::transport::smtp::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Address(_) => out.write_str("an address is not one the mail library accepts"),
            Self::Message(_) => out.write_str("the message could not be built"),
            Self::Transport(_) => out.write_str("the mail server could not be reached or refused the message"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Address(source) => Some(source),
            Self::Message(source) => Some(source),
            Self::Transport(source) => Some(source),
        }
    }
}

impl From<lettre::address::AddressError> for Error {
    fn from(source: lettre::address::AddressError) -> Self {
        Self::Address(source)
    }
}

impl From<lettre::error::Error> for Error {
    fn from(source: lettre::error::Error) -> Self {
        Self::Message(source)
    }
}

impl From<lettre::transport::smtp::Error> for Error {
    fn from(source: lettre::transport::smtp::Error) -> Self {
        Self::Transport(source)
    }
}
