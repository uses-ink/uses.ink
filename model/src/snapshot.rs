//! What a source hands the engine: an immutable tree at one version, facts about its files, and
//! what the fetch could and could not do.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Announcement, AnyCode, Bytes, ContentPath, Hash, Notice, Timestamp};

mod version;

pub use version::{Oid, UploadId, Version, VersionError};

/// An immutable tree of files at one version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    version: Version,
    tree: Vec<Entry>,
    facts: Vec<FileFacts>,
    observed: Observed,
}

/// One file in a snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[non_exhaustive]
pub struct Entry {
    /// Where it is in the tree.
    pub path: ContentPath,
    /// The hash of its bytes.
    pub blob: Hash,
    /// How large it is.
    pub size: Bytes,
}

impl Entry {
    /// The file at `path`, with the bytes of `blob`.
    pub fn new(path: ContentPath, blob: Hash, size: Bytes) -> Self {
        Self { path, blob, size }
    }
}

/// What a source knows about one file beyond its bytes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
pub struct FileFacts {
    /// The file.
    pub path: ContentPath,
    /// When it came to be at its path: the latest commit that added it, so a file deleted and
    /// added again was created when it came back, and a file moved there when it moved.
    pub created: Known<Timestamp>,
    /// When it was last changed.
    pub updated: Known<Timestamp>,
}

impl FileFacts {
    /// What is known of `path`: when it was added and last changed.
    pub fn new(path: ContentPath, created: Known<Timestamp>, updated: Known<Timestamp>) -> Self {
        Self { path, created, updated }
    }
}

/// A fact with how sure the source is of it (decision 0011).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Known<T> {
    /// The value itself.
    Exact {
        /// The value.
        value: T,
        /// Where it came from.
        from: Provenance,
    },
    /// A bound: the true value is this or earlier. Never treated as exact.
    NotAfter {
        /// The latest the value can be.
        value: T,
        /// Where it came from.
        from: Provenance,
    },
    /// Nothing is known.
    Unknown {
        /// Why not.
        reason: UnknownReason,
    },
}

impl<T> Known<T> {
    /// The value, only when it is exact.
    pub fn exact(&self) -> Option<&T> {
        match self {
            Self::Exact { value, .. } => Some(value),
            Self::NotAfter { .. } | Self::Unknown { .. } => None,
        }
    }
}

/// Where a fact came from.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Provenance {
    /// Git history, read up to this commit: a bound at a shallow clone's boundary.
    GitHistory {
        /// The commit history was read up to.
        commit: Oid,
    },
    /// The git commit that made the change.
    Commit {
        /// The commit.
        commit: Oid,
    },
    /// The local filesystem's metadata.
    Filesystem,
}

/// Why a fact is unknown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum UnknownReason {
    /// The source has no history ability.
    NoHistory,
    /// The history was cut at this many commits before the file's start.
    BeyondDepth {
        /// How many commits were read.
        depth: u32,
    },
    /// The source could have said and did not.
    NotReported,
    /// The history shows the change only in a merge commit, and merges are not read
    /// (decision 0036).
    Merged,
}

/// Something a source can do (decision 0012).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Ability {
    /// It can give an immutable tree at one version.
    Snapshot,
    /// It knows when each file was added and changed.
    History,
    /// It can say when its content changes.
    Watch,
    /// Its content is not public: nothing is published until it is selected.
    Private,
}

/// What one fetch actually had, and what it lacked.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
pub struct Observed {
    /// What it could do.
    pub has: Vec<Ability>,
    /// What it could not do, each with the decision that suffers for it.
    pub lacks: Vec<Lack>,
}

impl Observed {
    /// A fetch that had `has` and lacked `lacks`.
    pub fn new(has: Vec<Ability>, lacks: Vec<Lack>) -> Self {
        Self { has, lacks }
    }
}

/// An ability a fetch did not have, or had only in part, and the decision that suffers for it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
pub struct Lack {
    /// What the fetch could not do.
    pub ability: Ability,
    /// The decision that suffers for it, such as `page.date` without history.
    pub consequence: AnyCode,
    /// How much of the ability is missing.
    pub gap: Gap,
}

impl Lack {
    /// `ability` lacked as far as `gap` says, which `consequence` suffers for.
    pub fn new(ability: Ability, consequence: AnyCode, gap: Gap) -> Self {
        Self { ability, consequence, gap }
    }

    /// What a build says of it. A lack of history is said, since dates suffer for it; a
    /// source without `watch` or `private` builds the same, so that lack is shown by
    /// `ink explain` and not said.
    pub fn notice(&self) -> Option<Notice> {
        let decision = self.consequence;
        let announcement = match (&self.gap, self.ability) {
            (Gap::Absent, Ability::History) => Announcement::SourceHistoryAbsent { decision },
            (Gap::Absent, _) => return None,
            (Gap::Shallow { commit, bound }, _) => {
                Announcement::SourceHistoryShallow { decision, commit: commit.to_string(), bound: bound.to_string() }
            }
            (Gap::Renames { moved }, _) => Announcement::SourceHistoryMoved { decision, count: *moved },
        };
        Some(Notice::new(announcement))
    }
}

/// How much of an ability a fetch lacked.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Gap {
    /// All of it: a folder has no history at all.
    Absent,
    /// History stops at a shallow clone's boundary: what is older is only a bound.
    Shallow {
        /// The newest boundary commit.
        commit: Oid,
        /// Its author time, the bound every older fact gets.
        bound: Timestamp,
    },
    /// History does not follow renames (decision 0036): a file moved to its path was created,
    /// as far as the source knows, when it moved.
    Renames {
        /// How many files of the snapshot were created by a move.
        moved: u64,
    },
}

/// Why parts do not make a [`Snapshot`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SnapshotError {
    /// Two entries share a path.
    Duplicate {
        /// The path.
        path: ContentPath,
    },
    /// Two fact records share a path.
    DuplicateFacts {
        /// The path.
        path: ContentPath,
    },
    /// Facts about a path the tree does not hold.
    Stray {
        /// The path.
        path: ContentPath,
    },
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate { path } => write!(out, "the tree holds `{path}` twice"),
            Self::DuplicateFacts { path } => write!(out, "two sets of facts for `{path}`"),
            Self::Stray { path } => write!(out, "facts for `{path}`, which the tree does not hold"),
        }
    }
}

impl std::error::Error for SnapshotError {}

impl Snapshot {
    /// The entries and facts are sorted by path; a path twice, or facts about a path not in the
    /// tree, is an error.
    pub fn new(
        version: Version,
        mut tree: Vec<Entry>,
        mut facts: Vec<FileFacts>,
        observed: Observed,
    ) -> Result<Self, SnapshotError> {
        tree.sort_by(|a, b| a.path.cmp(&b.path));
        for pair in tree.windows(2) {
            if let [a, b] = pair
                && a.path == b.path
            {
                return Err(SnapshotError::Duplicate { path: a.path.clone() });
            }
        }
        facts.sort_by(|a, b| a.path.cmp(&b.path));
        let mut previous: Option<&ContentPath> = None;
        for fact in &facts {
            if previous == Some(&fact.path) {
                return Err(SnapshotError::DuplicateFacts { path: fact.path.clone() });
            }
            if tree.binary_search_by(|entry| entry.path.cmp(&fact.path)).is_err() {
                return Err(SnapshotError::Stray { path: fact.path.clone() });
            }
            previous = Some(&fact.path);
        }
        Ok(Self { version, tree, facts, observed })
    }

    /// The version it was taken at.
    pub fn version(&self) -> &Version {
        &self.version
    }

    /// Every file, sorted by path, each path once.
    pub fn tree(&self) -> &[Entry] {
        &self.tree
    }

    /// Facts, sorted by path, at most one record per path.
    pub fn facts(&self) -> &[FileFacts] {
        &self.facts
    }

    /// What the fetch could and could not do.
    pub fn observed(&self) -> &Observed {
        &self.observed
    }

    /// The entry at `path`.
    pub fn entry(&self, path: &ContentPath) -> Option<&Entry> {
        let at = self.tree.binary_search_by(|entry| entry.path.cmp(path)).ok()?;
        self.tree.get(at)
    }

    /// The facts about `path`.
    pub fn facts_of(&self, path: &ContentPath) -> Option<&FileFacts> {
        let at = self.facts.binary_search_by(|fact| fact.path.cmp(path)).ok()?;
        self.facts.get(at)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn entry(path: &str) -> Entry {
        Entry::new(ContentPath::parse(path).unwrap(), Hash::of(path), Bytes(1))
    }

    fn folder() -> Version {
        Version::Folder { hash: Hash::of(b"") }
    }

    #[test]
    fn sorts_its_tree() {
        let snapshot =
            Snapshot::new(folder(), vec![entry("b.md"), entry("a.md")], vec![], Observed::default()).unwrap();
        let paths: Vec<&str> = snapshot.tree().iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["a.md", "b.md"]);
        assert!(snapshot.entry(&ContentPath::parse("b.md").unwrap()).is_some());
    }

    #[test]
    fn an_object_id_is_its_bytes_in_hex() {
        let oid = Oid::try_from(&[0xab; 20][..]).unwrap();
        assert_eq!(oid.as_str(), "ab".repeat(20));
        assert_eq!(Oid::parse(oid.as_str()), Ok(oid));
        assert!(Oid::try_from(&[0; 19][..]).is_err());
    }

    #[test]
    fn refuses_a_path_twice_and_stray_facts() {
        let twice = Snapshot::new(folder(), vec![entry("a.md"), entry("a.md")], vec![], Observed::default());
        assert!(matches!(twice, Err(SnapshotError::Duplicate { .. })));
        let unknown = Known::Unknown { reason: UnknownReason::NoHistory };
        let stray = FileFacts::new(ContentPath::parse("z.md").unwrap(), unknown.clone(), unknown);
        let result = Snapshot::new(folder(), vec![entry("a.md")], vec![stray], Observed::default());
        assert!(matches!(result, Err(SnapshotError::Stray { .. })));
    }
}
