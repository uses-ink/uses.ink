//! What to fetch, how, and the handle a fetch gives back.

use super::{Grant, Locator, Revision, Scheme};
use crate::{Bytes, Hash, Oid};
use std::fmt;
use std::num::NonZeroU32;

/// How much history to fetch (decision 0036).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Depth {
    /// Every commit: a clone in full, which is the road to history. A shallow clone already in
    /// the cache is deepened to it.
    Full,
    /// Only this many commits from the tip: a snapshot first, or a bounded window. Dates of files
    /// older than the window are bounds.
    Commits(NonZeroU32),
}

/// One fetch.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Request {
    /// The repository.
    pub locator: Locator,
    /// The branch, tag or commit to read; the repository's own `HEAD` when none is named.
    pub revision: Option<Revision>,
    /// How much history.
    pub depth: Depth,
    /// The credential to fetch with, when the repository is private.
    pub grant: Option<Grant>,
}

impl Request {
    /// The repository's place in the host's cache: the hash of the address, revision and grant as
    /// written, so two revisions of one repository are two clones, and a clone fetched with one
    /// credential is never the one a request with another (or none) reads.
    pub fn key(&self) -> Hash {
        let revision = self.revision.as_ref().map_or_else(String::new, ToString::to_string);
        let grant = self.grant.as_ref().map_or_else(String::new, ToString::to_string);
        Hash::of(format!("{}\n{revision}\n{grant}", self.locator))
    }

    /// The whole history of the repository at `locator`, at its `HEAD`, with no credential.
    pub fn new(locator: Locator) -> Self {
        Self { locator, revision: None, depth: Depth::Full, grant: None }
    }
}

/// Whether a site's dates come from history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum History {
    /// They do: the source fetches every commit, so each file's dates are the commits that made
    /// them.
    #[default]
    Wanted,
    /// They do not: one commit is enough, and every date is a bound.
    Unneeded,
}

/// What a source plugin is asked to read: a repository, and how it will be used. The plugin
/// decides what that takes to fetch.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Want {
    /// The repository.
    pub locator: Locator,
    /// The branch, tag or commit to read; the repository's `HEAD` when none is named.
    pub revision: Option<Revision>,
    /// Whether dates come from history.
    pub history: History,
    /// The credential to fetch with, when the repository is private.
    pub grant: Option<Grant>,
}

impl Want {
    /// The repository at `locator`, at its `HEAD`, with its history, and no credential.
    pub fn new(locator: Locator) -> Self {
        Self { locator, revision: None, history: History::Wanted, grant: None }
    }
}

/// What the host holds after a fetch: a handle to the repository it cached, and the commit the
/// request's revision named in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cloned {
    /// The repository's place in the host's cache: the [`Request::key`].
    pub clone: Hash,
    /// The commit the revision names, which a snapshot of the clone is taken at.
    pub commit: Oid,
    /// Whether the history is cut short of the root commit.
    pub shallow: bool,
    /// How many objects the clone holds.
    pub objects: u64,
    /// How large the clone is on the host's disk.
    pub bytes: Bytes,
}

/// What the builder lets a plugin fetch (an operator's limit, decision 0044).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Network {
    /// No repository is fetched, not even one on the builder's own disk.
    #[default]
    Off,
    /// Only repositories on the builder's own disk, `file://`.
    Local,
    /// Every scheme the builder has a transport for.
    On,
}

impl Network {
    /// How the setting is named.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Local => "local",
            Self::On => "on",
        }
    }

    /// Whether a repository at `scheme` may be fetched.
    pub const fn allows(self, scheme: Scheme) -> bool {
        matches!((self, scheme), (Self::On, _) | (Self::Local, Scheme::File))
    }
}

impl fmt::Display for Network {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// A budget a fetch passed, with the number and the limit (architecture.md section 13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Budget {
    /// The clone is larger than a source may be: `source.budget.bytes`.
    Bytes {
        /// How large it was when the fetch stopped.
        size: Bytes,
        /// The most it may be.
        limit: Bytes,
    },
    /// The clone holds more objects than a source may: `source.budget.objects`.
    Objects {
        /// How many it held.
        count: u64,
        /// The most it may hold.
        limit: u64,
    },
}

/// Why a credential was not used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Auth {
    /// The request names a credential the host does not hold.
    Absent,
}
