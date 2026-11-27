//! A git repository on disk read into a snapshot at one commit, with dates from its history.
//!
//! The snapshot is the commit's tree, never the working copy: uncommitted files are not part of
//! a version, and a version is what a snapshot is taken at. The tree is read with the folder's
//! budgets and refusals: a link to a file of the tree is that file, a link to a folder of the tree
//! is skipped and said, a link out of the tree is refused. A submodule is skipped and said. A
//! Git LFS pointer is kept as the pointer's text and said.
//!
//! Dates come from one walk over the history behind the commit (decision 0036): each commit
//! that is not a merge is diffed against its parent, on a few threads, and the hits are folded
//! newest author time first. A file's `updated` is the newest commit that touched its path,
//! its `created` the newest commit that added it, each [`Known::Exact`](model::Known) and
//! naming that commit. Renames are not followed: a file moved to its path was created when it
//! moved, and the snapshot's [`Observed`] says how many files that was. A shallow clone cuts the
//! history; a file not touched since its boundary gets a [`Known::NotAfter`](model::Known) bound
//! at the boundary, and the snapshot says what is lacking.

mod history;
mod link;
mod tree;

use std::num::NonZeroUsize;
use std::path::PathBuf;

use gix::ObjectId;
use gix::odb::pack::cache::lru::MemoryCappedHashmap;
use model::{Ability, ContentPath, Observed, Oid, Snapshot, Version};

use crate::blob::Store;
use crate::budget::Budget;
use crate::error::Refused;
use crate::read::Read;

/// A git repository to read at one commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    /// The repository's working folder, or its git folder.
    pub root: PathBuf,
    /// The commit to read.
    pub revision: Revision,
    /// How much of the commit's tree may be read.
    pub budget: Budget,
    /// Folders of the tree holding the build's own output, not read.
    pub output: Vec<ContentPath>,
    /// What reading may spend; set by the operator of the build.
    pub limits: Limits,
}

/// What one read of a repository's history spends, and the longest chain of links it follows.
/// Set by the operator of the build, never by the site being built. None of it changes a byte of
/// the snapshot, except `hops`, which decides whether a chain of links resolves or is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// How many threads diff the history. The dates are the same on any number.
    pub threads: NonZeroUsize,
    /// Bytes of delta bases each thread keeps inflated: on tldr, 4 MiB to 32 MiB all take the
    /// dates from 4.6 s to 2.7 s on 4 threads; 8 MiB costs 55 MiB more at its peak.
    pub deltas: NonZeroUsize,
    /// Links followed before a chain counts as a loop, as Linux's `ELOOP` does.
    pub hops: usize,
    /// Commits handed to a diffing thread at a time.
    pub block: NonZeroUsize,
}

impl Default for Limits {
    /// Up to four threads (the spike's measured sweet spot, fewer when the machine has fewer),
    /// 8 MiB of delta bases a thread, 40 hops and blocks of 256 commits.
    fn default() -> Self {
        let four = NonZeroUsize::MIN.saturating_add(3);
        let threads = std::thread::available_parallelism().map_or(four, |cores| cores.min(four));
        Self {
            threads,
            deltas: NonZeroUsize::MIN.saturating_add((8 << 20) - 1),
            hops: 40,
            block: NonZeroUsize::MIN.saturating_add(255),
        }
    }
}

/// Which commit a [`Repository`] is read at.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Revision {
    /// The commit `HEAD` names. In a jj workspace, that is the parent of the working copy.
    #[default]
    Head,
    /// A revision as `git rev-parse` reads it: a branch, a tag, a commit id or `HEAD~2`.
    Spec(String),
}

impl Repository {
    /// The repository at `root`, read at `HEAD` with the v1 budget and the default [`Limits`].
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let (revision, budget, output, limits) = (Revision::Head, Budget::default(), Vec::new(), Limits::default());
        Self { root: root.into(), revision, budget, output, limits }
    }

    /// Reads the commit's tree into `store` and the snapshot, with every file's dates from the
    /// history behind it. Nothing is read in part: a refusal stops the read.
    pub fn read(&self, store: &mut impl Store) -> Result<Read, Refused> {
        let repo = self.open()?;
        let commit = self.commit(&repo)?;
        let tree = commit.tree_id().map_err(|source| Refused::Git { source })?.detach();
        let walked = tree::Walk::new(&repo, self, store).read(tree)?;
        let paths: Vec<&ContentPath> = walked.entries.iter().map(|entry| &entry.path).collect();
        let history = history::History { repo: &repo, tip: commit.id, limits: self.limits };
        let dates = history.dates(&paths)?;
        let observed = Observed::new(vec![Ability::Snapshot, Ability::History], dates.lacks);
        let version = Version::GitCommit(Id(commit.id).try_into()?);
        let snapshot = Snapshot::new(version, walked.entries, dates.facts, observed).map_err(Refused::Snapshot)?;
        Ok(Read { snapshot, skipped: walked.skipped, pointers: walked.pointers })
    }

    /// Opens the repository at `root` and no other: no search upwards, and no configuration but
    /// its own, so the user's and the system's settings change nothing.
    fn open(&self) -> Result<gix::Repository, Refused> {
        let mut repo = gix::open_opts(&self.root, gix::open::Options::isolated())
            .map_err(|source| Refused::Repository { path: self.root.clone(), source })?;
        repo.object_cache_size(0);
        // gix sets a delta-base cache from `core.deltaBaseCacheLimit` only with its
        // `max-performance` feature, which pulls MPL code; so it is set here, and every thread's
        // handle makes its own (decision 0036, as measured)
        let deltas = self.limits.deltas.get();
        repo.objects.set_pack_cache(move || Box::new(MemoryCappedHashmap::new(deltas)));
        Ok(repo)
    }

    /// The commit [`Repository::revision`] names, peeled through tags.
    fn commit<'r>(&self, repo: &'r gix::Repository) -> Result<gix::Commit<'r>, Refused> {
        match &self.revision {
            Revision::Head => repo.head_commit().map_err(|source| Refused::Revision { written: "HEAD".into(), source }),
            Revision::Spec(written) => {
                let refused = |source| Refused::Revision { written: written.clone(), source };
                let id = repo.rev_parse_single(written.as_str()).map_err(refused)?;
                id.object().and_then(gix::Object::peel_to_commit).map_err(refused)
            }
        }
    }
}

/// A git object id, on its way to the model's [`Oid`].
#[derive(Clone, Copy, Debug)]
struct Id(ObjectId);

impl TryFrom<Id> for Oid {
    type Error = Refused;

    fn try_from(Id(id): Id) -> Result<Self, Self::Error> {
        Oid::try_from(id.as_bytes()).map_err(|source| Refused::Id { source })
    }
}
