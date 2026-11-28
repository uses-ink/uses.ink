//! What claiming, proving and releasing a repository fail with.

use std::fmt;

use authority_forge::Unproven;
use control::Ports;

/// A repository call that failed. `T` is the transport's own error and `D` the random source's.
pub enum Error<P: Ports, T, D> {
    /// The random source gave no bytes.
    Draw(D),
    /// The forge did not show the challenge ([`Unproven`]).
    Unproven(Unproven<T>),
    /// The control plane refused or failed ([`control::Error`]).
    Control(control::Error<P>),
}

impl<P: Ports, T: fmt::Debug, D: fmt::Debug> fmt::Debug for Error<P, T, D> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draw(error) => out.debug_tuple("Draw").field(error).finish(),
            Self::Unproven(error) => out.debug_tuple("Unproven").field(error).finish(),
            Self::Control(error) => out.debug_tuple("Control").field(error).finish(),
        }
    }
}

impl<P: Ports, T, D> fmt::Display for Error<P, T, D> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draw(_) => out.write_str("no random bytes were available for the claim"),
            Self::Unproven(_) => out.write_str("the repository did not prove the challenge"),
            Self::Control(_) => out.write_str("the control plane did not take the repository"),
        }
    }
}

impl<P: Ports + 'static, T: std::error::Error + 'static, D: std::error::Error + 'static> std::error::Error
    for Error<P, T, D>
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Draw(source) => Some(source),
            Self::Unproven(source) => Some(source),
            Self::Control(source) => Some(source),
        }
    }
}

impl<P: Ports, T, D> From<control::Error<P>> for Error<P, T, D> {
    fn from(source: control::Error<P>) -> Self {
        Self::Control(source)
    }
}

impl<P: Ports, T, D> Error<P, T, D> {
    /// The code of the catalog that words this, when it has one: the forge's, or the control
    /// plane's refusal. A failure of a port has none.
    pub const fn code(&self) -> Option<&'static str> {
        match self {
            Self::Draw(_) => None,
            Self::Unproven(error) => Some(error.code()),
            Self::Control(error) => match error.refusal() {
                Some(refusal) => Some(refusal.code()),
                None => None,
            },
        }
    }
}
