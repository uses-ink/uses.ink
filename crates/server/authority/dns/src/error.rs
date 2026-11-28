//! What proving a name fails with.

use std::fmt;

use control::Ports;

/// A proof that could not be checked or was refused. `R` is the resolver's own error.
pub enum Error<P: Ports, R> {
    /// The lookup got no answer. It says nothing about the name.
    Lookup(R),
    /// The control plane refused or failed ([`control::Error`]).
    Control(control::Error<P>),
}

impl<P: Ports, R: fmt::Debug> fmt::Debug for Error<P, R> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lookup(error) => out.debug_tuple("Lookup").field(error).finish(),
            Self::Control(error) => out.debug_tuple("Control").field(error).finish(),
        }
    }
}

impl<P: Ports, R> fmt::Display for Error<P, R> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lookup(_) => out.write_str("the DNS lookup of the proof record got no answer"),
            Self::Control(_) => out.write_str("the control plane did not take the proof"),
        }
    }
}

impl<P: Ports + 'static, R: std::error::Error + 'static> std::error::Error for Error<P, R> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lookup(source) => Some(source),
            Self::Control(source) => Some(source),
        }
    }
}

impl<P: Ports, R> From<control::Error<P>> for Error<P, R> {
    fn from(source: control::Error<P>) -> Self {
        Self::Control(source)
    }
}

impl<P: Ports, R> Error<P, R> {
    /// The code of the catalog (`crates/core/diag/catalog.toml`) that words this: the lookup's
    /// own, or the control plane's refusal. A failure of a port has none.
    pub const fn code(&self) -> Option<&'static str> {
        match self {
            Self::Lookup(_) => Some("domain.lookup.failed"),
            Self::Control(error) => match error.refusal() {
                Some(refusal) => Some(refusal.code()),
                None => None,
            },
        }
    }
}
