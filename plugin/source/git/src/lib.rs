//! `ink:source-git`: a git repository named by its address, read into a snapshot (decision 0046).
//!
//! The rule is here, in plain Rust, written against the host's `git` capability ([`Git`]):
//! [`Source::read`] chooses how much history to fetch from what the site needs (every commit when
//! its dates come from history, one when they do not), fetches, asks the host for the snapshot
//! of what it fetched, and refuses a snapshot that is not of that commit. The host holds the
//! clone, the network, the credentials and the files' bytes; the snapshot, with `Known` dates
//! and the abilities it observed, is what comes back.
//!
//! The same rule runs natively, over the host's own implementation, and as a component, over the
//! imported one: `uses-ink-source-git-guest` is the glue, and the two must give the same
//! snapshot.

use std::fmt;

use model::git::{Depth, Git, History, Refused, Request, Want};
use model::{Oid, Snapshot, Version};

/// The git source, reading through the host capability `G`.
#[derive(Debug)]
pub struct Source<G>(pub G);

/// Why a repository was not read.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unread<E> {
    /// The host refused or failed.
    Refused(Refused<E>),
    /// The host's snapshot is of another version than the commit it fetched: a bug in the host.
    Version {
        /// The commit fetched.
        fetched: Oid,
        /// The version the snapshot says.
        found: Version,
    },
}

impl<E: fmt::Display> fmt::Display for Unread<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(refused) => write!(out, "{refused}"),
            Self::Version { fetched, .. } => {
                write!(out, "the snapshot is not of the fetched commit {}", fetched.as_str())
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Unread<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Refused(refused) => Some(refused),
            Self::Version { .. } => None,
        }
    }
}

impl<E> From<Refused<E>> for Unread<E> {
    fn from(refused: Refused<E>) -> Self {
        Self::Refused(refused)
    }
}

/// How much to fetch for a [`Want`]: decision 0036 fetches every commit when history is wanted,
/// and depth 1 when only the snapshot is.
struct Plan<'w>(&'w Want);

impl Plan<'_> {
    /// The commits fetched when no history is needed.
    const SNAPSHOT: Depth = match std::num::NonZeroU32::new(1) {
        Some(one) => Depth::Commits(one),
        None => Depth::Full,
    };

    fn request(&self) -> Request {
        let depth = match self.0.history {
            History::Unneeded => Self::SNAPSHOT,
            _ => Depth::Full,
        };
        let mut request = Request::new(self.0.locator.clone());
        (request.revision, request.depth, request.grant) = (self.0.revision.clone(), depth, self.0.grant.clone());
        request
    }
}

impl<G: Git> Source<G> {
    /// The snapshot of the repository `want` names, its files stored by the host.
    pub fn read(&mut self, want: &Want) -> Result<Snapshot, Unread<G::Error>> {
        let cloned = self.0.fetch(&Plan(want).request())?;
        let snapshot = self.0.snapshot(&cloned)?;
        match snapshot.version() {
            Version::GitCommit(commit) if *commit == cloned.commit => Ok(snapshot),
            found => Err(Unread::Version { fetched: cloned.commit, found: found.clone() }),
        }
    }
}
