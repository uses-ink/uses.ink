//! Everything the host refuses, with its numbers.

use super::{Auth, Budget, Grant, Network, Revision, Scheme};
use crate::{Diagnostic, Problem};
use std::fmt;

/// What the host refused of a fetch or a read. `E` is the cause of a failure with no case of its
/// own: in the host, the error itself; in a plugin, a marker, since the host has already said
/// why.
#[derive(Debug)]
#[non_exhaustive]
pub enum Refused<E> {
    /// The builder's network setting forbids the scheme.
    Network {
        /// The scheme asked for.
        scheme: Scheme,
        /// The setting that forbids it.
        policy: Network,
    },
    /// The credential asked for was not used.
    Auth {
        /// The credential's name.
        grant: Grant,
        /// Why not.
        outcome: Auth,
    },
    /// A budget was passed, and the clone was removed.
    Budget(Budget),
    /// The revision names nothing the clone holds.
    Revision {
        /// The revision, as asked.
        written: Revision,
    },
    /// The builder has no transport for the scheme.
    Transport {
        /// The scheme.
        scheme: Scheme,
    },
    /// Anything else: an unreachable remote, a damaged clone, a disk that refused.
    Failed(E),
}

impl<E: fmt::Display> fmt::Display for Refused<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network { scheme, policy } => {
                write!(out, "{scheme} repositories are not fetched: network is {policy}")
            }
            Self::Auth { grant, .. } => write!(out, "the credential `{grant}` is not held"),
            Self::Budget(Budget::Bytes { size, limit }) => {
                write!(out, "the clone is {size}; a source may hold {limit}")
            }
            Self::Budget(Budget::Objects { count, limit }) => {
                write!(out, "the clone holds {count} objects; a source may hold {limit}")
            }
            Self::Revision { written } => write!(out, "`{written}` names nothing the clone holds"),
            Self::Transport { scheme } => write!(out, "there is no transport for {scheme} repositories"),
            Self::Failed(error) => write!(out, "{error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Refused<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Failed(error) => Some(error),
            _ => None,
        }
    }
}

impl<E: std::error::Error> From<&Refused<E>> for Diagnostic {
    /// The refusal as the author or operator sees it. A failure's causes are written out once
    /// here, at the edge, as the catalog's `reason`.
    fn from(refused: &Refused<E>) -> Self {
        Self::new(match refused {
            Refused::Network { scheme, policy } => {
                Problem::SourceGitNetwork { scheme: scheme.to_string(), policy: policy.to_string() }
            }
            Refused::Auth { grant, .. } => Problem::SourceGitAuth { grant: grant.to_string() },
            Refused::Budget(Budget::Bytes { size, limit }) => Problem::SourceBudgetBytes { size: *size, limit: *limit },
            Refused::Budget(Budget::Objects { count, limit }) => {
                Problem::SourceBudgetObjects { count: *count, limit: *limit }
            }
            Refused::Revision { written } => Problem::SourceGitRevision { written: written.to_string() },
            Refused::Transport { scheme } => Problem::SourceGitTransport { scheme: scheme.to_string() },
            Refused::Failed(error) => Problem::SourceRepositoryUnreadable { reason: Chain(error).to_string() },
        })
    }
}

/// An error and its causes as one line, for a diagnostic's `reason`.
struct Chain<'e, E>(&'e E);

impl<E: std::error::Error> fmt::Display for Chain<'_, E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)?;
        let mut cause = self.0.source();
        while let Some(error) = cause {
            write!(out, ": {error}")?;
            cause = error.source();
        }
        Ok(())
    }
}
