//! Why a server does not start.

use std::fmt;

/// A server that cannot be built or run.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The edge's health route is not a route.
    Probe(model::RouteError),
    /// The request-head limit is below what hyper can read.
    Head {
        /// The limit the operator set.
        limit: u64,
        /// The least hyper reads.
        least: u64,
    },
    /// The listener has no address.
    Address(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Probe(_) => write!(out, "the health route is not a route"),
            Self::Head { limit, least } => {
                write!(out, "the request head limit is {limit} bytes; hyper reads at least {least}")
            }
            Self::Address(_) => write!(out, "the listener has no address"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Probe(source) => Some(source),
            Self::Head { .. } => None,
            Self::Address(source) => Some(source),
        }
    }
}
