//! A folder on disk read into a snapshot.

use std::collections::BTreeMap;
use std::fs::Metadata;
use std::path::{Path, PathBuf};

use model::{
    Ability, AnyCode, Bytes, ContentPath, Entry, FileFacts, Gap, Hash, Known, Lack, Observed, Provenance, Snapshot,
    Timestamp, UnknownReason, Version, Written,
};

use crate::blob::Store;
use crate::budget::Budget;
use crate::cache::Disk;
use crate::error::Refused;
use crate::handle::{Handle, Identity};
use crate::read::{Read, Skip, Skipped};

/// Folders of version control, and the one `ink init` keeps its link in, never part of a
/// snapshot and never said.
const VERSION_CONTROL: &[&str] = &[".git", ".jj", ".uses"];

/// What [`Version::Folder`] hashes, before the sorted tree.
const PREFIX: &[u8] = b"uses.ink/folder/v1\n";

/// A folder to read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    /// The folder.
    pub root: PathBuf,
    /// How much of it may be read.
    pub budget: Budget,
    /// Folders inside it holding the build's own output, not read.
    pub output: Vec<ContentPath>,
}

/// The walk so far.
#[derive(Debug)]
struct Walk<'a, S> {
    folder: &'a Folder,
    store: &'a mut S,
    entries: Vec<Entry>,
    facts: Vec<FileFacts>,
    skipped: Vec<Skipped>,
    /// File entries met, links to a file among them.
    files: u64,
    /// Folders walked, the root among them.
    folders: u64,
    /// Bytes read, each file once.
    total: u64,
    /// Each file read so far, by identity, so a link to it is not read again.
    seen: BTreeMap<Identity, (Hash, Bytes)>,
}

impl Folder {
    /// The manifest `ink build` writes into its output folder. A folder holding one that
    /// starts with [`Folder::MARK`] is output, so no read takes it as content.
    pub const MANIFEST: &'static str = <Written>::MANIFEST;

    /// How a manifest `ink build` wrote starts: its schema, first.
    pub const MARK: &'static str = <Written>::MARK;

    /// A folder read with the v1 budget, skipping nothing but version control.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), budget: Budget::default(), output: Vec::new() }
    }

    /// Reads every file into `store` and the snapshot. The walk stops at the first budget
    /// passed, so a refusal's count is how far it got, never past the limit by more than one.
    pub fn read(&self, store: &mut impl Store) -> Result<Read, Refused> {
        let root =
            std::fs::canonicalize(&self.root).map_err(|source| Refused::Root { path: self.root.clone(), source })?;
        let mut walk = Walk {
            folder: self,
            store,
            entries: Vec::new(),
            facts: Vec::new(),
            skipped: Vec::new(),
            files: 0,
            folders: 0,
            total: 0,
            seen: BTreeMap::new(),
        };
        walk.walk(&root, &root, None)?;
        let Walk { entries, facts, skipped, .. } = walk;
        let version = Version::Folder { hash: Tree(&entries).hash() };
        let observed =
            Observed::new(vec![Ability::Snapshot], vec![Lack::new(Ability::History, AnyCode::Date, Gap::Absent)]);
        let snapshot = Snapshot::new(version, entries, facts, observed).map_err(Refused::Snapshot)?;
        Ok(Read { snapshot, skipped, pointers: Vec::new() })
    }
}

impl<S: Store> Walk<'_, S> {
    /// Walks the folder at `real`, whose content paths start with `prefix`.
    fn walk(&mut self, root: &Path, real: &Path, prefix: Option<&ContentPath>) -> Result<(), Refused> {
        self.folders += 1;
        if self.folders > self.folder.budget.folders {
            return Err(Refused::Folders { limit: self.folder.budget.folders });
        }
        let unreadable = |source| match prefix {
            Some(path) => Refused::Unreadable { path: path.clone(), source },
            None => Refused::Root { path: self.folder.root.clone(), source },
        };
        if let Some(path) = prefix {
            let marked = Marked(real);
            let reason = if marked.is_output() {
                Some(Skip::Output)
            } else if marked.is_cache() {
                Some(Skip::Cache)
            } else {
                None
            };
            if let Some(reason) = reason {
                self.skipped.push(Skipped { path: path.clone(), reason });
                return Ok(());
            }
        }
        let mut children: Vec<(String, PathBuf)> = Vec::new();
        for child in std::fs::read_dir(real).map_err(unreadable)? {
            let child = child.map_err(unreadable)?;
            let real = child.path();
            let name = child.file_name().into_string().map_err(|_| Refused::Encoding { path: real.clone() })?;
            children.push((name, real));
        }
        children.sort();
        for (name, child) in children {
            if VERSION_CONTROL.contains(&name.as_str()) {
                continue;
            }
            let own = ContentPath::parse(name.clone()).map_err(|source| Refused::Name { written: name, source })?;
            let path = prefix.map_or_else(|| own.clone(), |prefix| prefix.join(&own));
            self.child(root, &child, path)?;
        }
        Ok(())
    }

    fn child(&mut self, root: &Path, real: &Path, path: ContentPath) -> Result<(), Refused> {
        let unreadable = |source| Refused::Unreadable { path: path.clone(), source };
        let metadata = std::fs::symlink_metadata(real).map_err(unreadable)?;
        if metadata.is_dir() {
            if self.folder.output.contains(&path) {
                self.skipped.push(Skipped { path, reason: Skip::Output });
                return Ok(());
            }
            return self.walk(root, real, Some(&path));
        }
        let real = if metadata.file_type().is_symlink() {
            let target = std::fs::canonicalize(real).map_err(unreadable)?;
            if !target.starts_with(root) {
                return Err(Refused::Outside { path, target });
            }
            if std::fs::metadata(&target).map_err(unreadable)?.is_dir() {
                self.skipped.push(Skipped { path, reason: Skip::Link });
                return Ok(());
            }
            target
        } else {
            real.to_path_buf()
        };
        self.file(&real, path)
    }

    /// Reads the file at `real` through one handle: the handle says what it is and how long,
    /// and no more than a file's budget is read from it.
    fn file(&mut self, real: &Path, path: ContentPath) -> Result<(), Refused> {
        let budget = self.folder.budget;
        self.files += 1;
        if self.files > budget.files {
            return Err(Refused::Files { count: self.files, limit: budget.files });
        }
        let unreadable = |source| Refused::Unreadable { path: path.clone(), source };
        let handle = Handle::open(real).map_err(unreadable)?;
        let metadata = handle.metadata().clone();
        if !metadata.is_file() {
            return Err(Refused::Special { path });
        }
        let bound = Modified(&metadata).bound();
        let identity = Identity::from(&metadata);
        if let Some(&(blob, size)) = self.seen.get(&identity) {
            self.facts.push(FileFacts::new(path.clone(), bound.clone(), bound));
            self.entries.push(Entry::new(path, blob, size));
            return Ok(());
        }
        if metadata.len() > budget.file.count() {
            return Err(Refused::File { path, size: Bytes(metadata.len()), limit: budget.file });
        }
        let bytes = handle.bytes(budget.file.count().saturating_add(1)).map_err(unreadable)?;
        // The file may have grown since the handle said how long it was.
        let size = Bytes(bytes.len() as u64);
        if size > budget.file {
            return Err(Refused::File { path, size, limit: budget.file });
        }
        self.total = self.total.saturating_add(size.count());
        if self.total > budget.total.count() {
            return Err(Refused::Total { size: Bytes(self.total), limit: budget.total });
        }
        let blob = self.store.put(bytes).map_err(Refused::Store)?;
        self.seen.insert(identity, (blob, size));
        self.facts.push(FileFacts::new(path.clone(), bound.clone(), bound));
        self.entries.push(Entry::new(path, blob, size));
        Ok(())
    }
}

/// A folder, asked whether it holds a manifest `ink build` wrote or the mark of ink's cache.
#[derive(Clone, Copy, Debug)]
struct Marked<'a>(&'a Path);

impl Marked<'_> {
    /// Whether [`Folder::MANIFEST`] is a file here, not a link, starting with [`Folder::MARK`].
    fn is_output(self) -> bool {
        self.holds(Folder::MANIFEST, Folder::MARK)
    }

    /// Whether [`Disk::LABEL`] is a file here, not a link, starting with [`Disk::MARK`].
    fn is_cache(self) -> bool {
        self.holds(Disk::LABEL, Disk::MARK)
    }

    /// Whether `name` is a file here, not a link, starting with `mark`.
    fn holds(self, name: &str, mark: &str) -> bool {
        let path = self.0.join(name);
        let is_file = std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_file());
        let mark = mark.as_bytes();
        is_file
            && Handle::open(&path)
                .ok()
                .filter(|handle| handle.metadata().is_file())
                .and_then(|handle| handle.bytes(mark.len() as u64).ok())
                .is_some_and(|bytes| bytes == mark)
    }
}

/// A file's modification time.
#[derive(Clone, Copy, Debug)]
struct Modified<'a>(&'a Metadata);

impl Modified<'_> {
    /// A bound on both when the file was made and when it last changed.
    fn bound(self) -> Known<Timestamp> {
        let at = self.0.modified().ok().and_then(|at| Timestamp::try_from(at).ok());
        match at {
            Some(value) => Known::NotAfter { value, from: Provenance::Filesystem },
            None => Known::Unknown { reason: UnknownReason::NotReported },
        }
    }
}

/// The files of a snapshot, for its version.
#[derive(Clone, Copy, Debug)]
struct Tree<'a>(&'a [Entry]);

impl Tree<'_> {
    /// The hash of the tree, sorted by path: [`PREFIX`], the number of entries, then each path
    /// as its length (64-bit little-endian) and bytes, its blob's 32 bytes and its size.
    fn hash(self) -> Hash {
        let mut sorted: Vec<&Entry> = self.0.iter().collect();
        sorted.sort_by(|a, b| a.path.cmp(&b.path));
        let mut bytes = PREFIX.to_vec();
        bytes.extend_from_slice(&(sorted.len() as u64).to_le_bytes());
        for entry in sorted {
            bytes.extend_from_slice(&(entry.path.as_str().len() as u64).to_le_bytes());
            bytes.extend_from_slice(entry.path.as_str().as_bytes());
            bytes.extend_from_slice(entry.blob.as_bytes());
            bytes.extend_from_slice(&entry.size.count().to_le_bytes());
        }
        Hash::of(bytes)
    }
}
