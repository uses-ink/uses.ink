//! What proving a repository or calling GitHub fails with.

use std::fmt;

use authority_forge::{Fail, Host, NameError, Segment};

use crate::jwt::Unsigned;

/// A proof or a call that failed. `S` is the signer's error and `T` the transport's.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error<S, T> {
    /// The JWT could not be signed.
    Sign(Unsigned<S>),
    /// A call to GitHub failed.
    Call(Fail<T>),
    /// The repository is on a host that is not GitHub.
    Unlisted {
        /// The host.
        host: Host,
    },
    /// The App is not installed on the repository.
    NotInstalled,
    /// The installation is suspended.
    Suspended,
    /// The installation is on another account than the one that claimed the repository.
    Account {
        /// The account the installation is on.
        found: Segment,
    },
    /// GitHub named an account that is no name.
    Name(NameError),
}

impl<S, T> Error<S, T> {
    /// The code of the catalog that words this.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Sign(_) => "app.sign.failed",
            Self::Call(fail) => fail.code(),
            Self::Unlisted { .. } => "repo.host.unlisted",
            Self::NotInstalled => "repo.app.missing",
            Self::Suspended => "repo.app.suspended",
            Self::Account { .. } => "repo.account.mismatch",
            Self::Name(_) => "forge.reply.malformed",
        }
    }
}

impl<S, T> fmt::Display for Error<S, T> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sign(_) => out.write_str("the App's JWT could not be signed"),
            Self::Call(_) => out.write_str("GitHub could not be asked"),
            Self::Unlisted { host } => write!(out, "{host} is not GitHub"),
            Self::NotInstalled => out.write_str("the App is not installed on the repository"),
            Self::Suspended => out.write_str("the App's installation on the repository is suspended"),
            Self::Account { found } => {
                write!(out, "the installation is on `{found}`, not on the account that claimed it")
            }
            Self::Name(_) => out.write_str("GitHub named an account that is not a name"),
        }
    }
}

impl<S: std::error::Error + 'static, T: std::error::Error + 'static> std::error::Error for Error<S, T> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sign(source) => Some(source),
            Self::Call(source) => Some(source),
            Self::Name(source) => Some(source),
            Self::Unlisted { .. } | Self::NotInstalled | Self::Suspended | Self::Account { .. } => None,
        }
    }
}

impl<S, T> From<Unsigned<S>> for Error<S, T> {
    fn from(source: Unsigned<S>) -> Self {
        Self::Sign(source)
    }
}

impl<S, T> From<Fail<T>> for Error<S, T> {
    fn from(source: Fail<T>) -> Self {
        Self::Call(source)
    }
}

impl<S> fmt::Display for Unsigned<S> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(_) => out.write_str("the JWT claims could not be written"),
            Self::Sign(_) => out.write_str("the signer refused to sign"),
        }
    }
}

impl<S: std::error::Error + 'static> std::error::Error for Unsigned<S> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(source) => Some(source),
            Self::Sign(source) => Some(source),
        }
    }
}
