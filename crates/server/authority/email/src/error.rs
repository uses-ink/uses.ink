//! What proving an address fails with.

use std::fmt;

/// A proof that could not be sent or confirmed. `E` is the database adapter's error and `M` the
/// mailer's.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<E, M> {
    /// The sign-in rows refused or failed. A proof that is unknown, used or ended is
    /// [`control::signin::Refused::Unusable`], and the answer does not say which.
    Rows(control::signin::Error<E>),
    /// The mail could not be sent. The proof was stored and ends on its own.
    Mail(M),
}

impl<E, M> Error<E, M> {
    /// The refusal this is, if the proof was refused rather than failed.
    pub const fn refusal(&self) -> Option<&control::signin::Refused> {
        match self {
            Self::Rows(error) => error.refusal(),
            Self::Mail(_) => None,
        }
    }
}

impl<E, M> fmt::Display for Error<E, M> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rows(_) => out.write_str("the address proof could not be stored or spent"),
            Self::Mail(_) => out.write_str("the address proof could not be mailed"),
        }
    }
}

impl<E: std::error::Error + 'static, M: std::error::Error + 'static> std::error::Error for Error<E, M> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Rows(source) => Some(source),
            Self::Mail(source) => Some(source),
        }
    }
}

impl<E, M> From<control::signin::Error<E>> for Error<E, M> {
    fn from(source: control::signin::Error<E>) -> Self {
        Self::Rows(source)
    }
}
