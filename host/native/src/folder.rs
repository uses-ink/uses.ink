//! A folder on disk read into a snapshot.

use std::fs::Metadata;
use std::path::{Path, PathBuf};

use model::{
    Ability, Bytes, ContentPath, DecisionCode, Entry, FileFacts, Hash, Known, Lack, Observed, Provenance, Snapshot,
    Timestamp, UnknownReason, Version,
};

use crate::blob::Store;
use crate::budget::Budget;
use crate::error::Refused;

/// Folders of version control, never part of a snapshot and never said.
const VERSION_CONTROL: &[&str] = &[".git", ".jj"];

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

/// A folder read.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Read {
    /// The files, their facts and the folder's version.
    pub snapshot: Snapshot,
    /// Folders not read because they hold the build's output, in path order.
    pub skipped: Vec<Skipped>,
}

/// A folder left unread because the caller named it as the build's output.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Skipped {
    /// The folder.
    pub path: ContentPath,
}

/// The walk so far.
#[derive(Debug)]
struct Walk<'a, S> {
    folder: &'a Folder,
    store: &'a mut S,
    entries: Vec<Entry>,
    facts: Vec<FileFacts>,
    skipped: Vec<Skipped>,
    /// Files seen, read or not.
    count: u64,
    /// Bytes seen, read or not.
    total: u64,
    /// The folders being walked, canonical, outermost first: a link back into one is a cycle.
    open: Vec<PathBuf>,
}

impl Folder {
    /// A folder read with the v1 budget, skipping nothing but version control.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), budget: Budget::default(), output: Vec::new() }
    }

    /// Reads every file into `store` and the snapshot. Past the file or byte budget it keeps
    /// counting without reading, so the refusal says how far over the folder is.
    pub fn read(&self, store: &mut impl Store) -> Result<Read, Refused> {
        let root =
            std::fs::canonicalize(&self.root).map_err(|source| Refused::Root { path: self.root.clone(), source })?;
        let mut walk = Walk {
            folder: self,
            store,
            entries: Vec::new(),
            facts: Vec::new(),
            skipped: Vec::new(),
            count: 0,
            total: 0,
            open: Vec::new(),
        };
        walk.walk(&root, &root, None)?;
        let Walk { entries, facts, skipped, count, total, .. } = walk;
        if count > self.budget.files {
            return Err(Refused::Files { count, limit: self.budget.files });
        }
        if total > self.budget.total.count() {
            return Err(Refused::Total { size: Bytes(total), limit: self.budget.total });
        }
        let version = Version::Folder { hash: Tree(&entries).hash() };
        let observed = Observed::new(vec![Ability::Snapshot], vec![Lack::new(Ability::History, DecisionCode::Date)]);
        let snapshot = Snapshot::new(version, entries, facts, observed).map_err(Refused::Snapshot)?;
        Ok(Read { snapshot, skipped })
    }
}

impl<S: Store> Walk<'_, S> {
    /// Walks the folder at `real`, whose content paths start with `prefix`.
    fn walk(&mut self, root: &Path, real: &Path, prefix: Option<&ContentPath>) -> Result<(), Refused> {
        let unreadable = |source| match prefix {
            Some(path) => Refused::Unreadable { path: path.clone(), source },
            None => Refused::Root { path: self.folder.root.clone(), source },
        };
        let mut children: Vec<(String, PathBuf)> = Vec::new();
        for child in std::fs::read_dir(real).map_err(unreadable)? {
            let child = child.map_err(unreadable)?;
            let real = child.path();
            let name = child.file_name().into_string().map_err(|_| Refused::Encoding { path: real.clone() })?;
            children.push((name, real));
        }
        children.sort();
        self.open.push(real.to_path_buf());
        for (name, child) in children {
            if VERSION_CONTROL.contains(&name.as_str()) {
                continue;
            }
            let own = ContentPath::parse(name.clone()).map_err(|source| Refused::Name { written: name, source })?;
            let path = prefix.map_or_else(|| own.clone(), |prefix| prefix.join(&own));
            self.child(root, &child, path)?;
        }
        self.open.pop();
        Ok(())
    }

    fn child(&mut self, root: &Path, real: &Path, path: ContentPath) -> Result<(), Refused> {
        let unreadable = |source| Refused::Unreadable { path: path.clone(), source };
        let metadata = std::fs::symlink_metadata(real).map_err(unreadable)?;
        let (real, metadata) = if metadata.file_type().is_symlink() {
            let target = std::fs::canonicalize(real).map_err(unreadable)?;
            if !target.starts_with(root) {
                return Err(Refused::Outside { path, target });
            }
            if self.open.contains(&target) {
                return Err(Refused::Cycle { path, target });
            }
            let metadata = std::fs::metadata(&target).map_err(unreadable)?;
            (target, metadata)
        } else {
            (real.to_path_buf(), metadata)
        };
        if metadata.is_dir() {
            if self.folder.output.contains(&path) {
                self.skipped.push(Skipped { path });
                return Ok(());
            }
            return self.walk(root, &real, Some(&path));
        }
        if !metadata.is_file() {
            return Err(Refused::Special { path });
        }
        self.file(&real, path, &metadata)
    }

    fn file(&mut self, real: &Path, path: ContentPath, metadata: &Metadata) -> Result<(), Refused> {
        let budget = self.folder.budget;
        let size = Bytes(metadata.len());
        if size > budget.file {
            return Err(Refused::File { path, size, limit: budget.file });
        }
        self.count += 1;
        self.total = self.total.saturating_add(size.count());
        if self.count > budget.files || self.total > budget.total.count() {
            return Ok(());
        }
        let bytes = std::fs::read(real).map_err(|source| Refused::Unreadable { path: path.clone(), source })?;
        // The file may have grown since its size was read.
        let size = Bytes(bytes.len() as u64);
        if size > budget.file {
            return Err(Refused::File { path, size, limit: budget.file });
        }
        let blob = self.store.put(bytes).map_err(Refused::Store)?;
        let bound = Modified(metadata).bound();
        self.facts.push(FileFacts::new(path.clone(), bound.clone(), bound));
        self.entries.push(Entry::new(path, blob, size));
        Ok(())
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
