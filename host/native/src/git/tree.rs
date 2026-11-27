//! A commit's tree walked into entries, with the folder's budgets.

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt as _;
use std::path::PathBuf;

use gix::ObjectId;
use gix::bstr::{BStr, BString, ByteSlice as _};
use gix::objs::tree::EntryKind;
use model::{Bytes, ContentPath, Entry, Hash};

use super::Repository;
use super::link::{Landing, Links, Node, Unresolved};
use crate::blob::Store;
use crate::cache::Disk;
use crate::error::Refused;
use crate::folder::Folder;
use crate::read::{Pointer, Skip, Skipped};

/// The tree so far.
#[derive(Debug)]
pub(super) struct Walk<'a, S> {
    repo: &'a gix::Repository,
    source: &'a Repository,
    store: &'a mut S,
    /// Every path of the tree, for links to find what they point at.
    nodes: BTreeMap<String, Node>,
    /// Links, by path, with the text they hold, resolved once the whole tree is known.
    links: Vec<(ContentPath, BString)>,
    walked: Walked,
    /// File entries met, links among them.
    files: u64,
    /// Trees walked, the root among them.
    folders: u64,
    /// Bytes read, each blob once.
    total: u64,
    /// Each blob read so far, so a second path to it is not read again.
    seen: HashMap<ObjectId, (Hash, Bytes)>,
}

/// A tree read.
#[derive(Debug, Default)]
pub(super) struct Walked {
    /// Its files, links resolved to what they point at.
    pub(super) entries: Vec<Entry>,
    /// Folders not read: in the order the walk met them, then links to folders in path order.
    pub(super) skipped: Vec<Skipped>,
    /// Files that are Git LFS pointers.
    pub(super) pointers: Vec<ContentPath>,
}

impl<'a, S: Store> Walk<'a, S> {
    pub(super) fn new(repo: &'a gix::Repository, source: &'a Repository, store: &'a mut S) -> Self {
        Self {
            repo,
            source,
            store,
            nodes: BTreeMap::new(),
            links: Vec::new(),
            walked: Walked::default(),
            files: 0,
            folders: 0,
            total: 0,
            seen: HashMap::new(),
        }
    }

    /// Walks the tree `root`, then resolves its links. The walk stops at the first budget
    /// passed, as the folder's does.
    pub(super) fn read(mut self, root: ObjectId) -> Result<Walked, Refused> {
        self.tree(root, None)?;
        let links = std::mem::take(&mut self.links);
        for (path, target) in links {
            match Links(&self.nodes).land(&path, target.as_ref()) {
                Ok(Landing::File(id)) => {
                    let (blob, size) = self.blob(id, &path)?;
                    self.walked.entries.push(Entry::new(path, blob, size));
                }
                Ok(Landing::Folder) => self.walked.skipped.push(Skipped { path, reason: Skip::Link }),
                Err(Unresolved::Outside) => return Err(Refused::Outside { path, target: Text(&target).into() }),
                Err(Unresolved::Broken) => return Err(Refused::Broken { path, target: Text(&target).into() }),
            }
        }
        Ok(self.walked)
    }

    /// Walks the tree `id`, whose paths start with `prefix`.
    fn tree(&mut self, id: ObjectId, prefix: Option<&ContentPath>) -> Result<(), Refused> {
        self.folders += 1;
        let budget = self.source.budget;
        if self.folders > budget.folders {
            return Err(Refused::Folders { limit: budget.folders });
        }
        let git = |source| Refused::Git { source };
        let tree = self.repo.find_tree(id).map_err(git)?;
        let decoded = tree.decode().map_err(git)?;
        let mut children = Vec::with_capacity(decoded.entries.len());
        for entry in &decoded.entries {
            let name = Name { prefix, name: entry.filename }.parse()?;
            let path = prefix.map_or_else(|| name.clone(), |prefix| prefix.join(&name));
            children.push((path, entry.mode.kind(), entry.oid.to_owned()));
        }
        if let Some(path) = prefix {
            let reason = if self.holds(&children, Folder::MANIFEST, Folder::MARK)? {
                Some(Skip::Output)
            } else if self.holds(&children, Disk::LABEL, Disk::MARK)? {
                Some(Skip::Cache)
            } else {
                None
            };
            if let Some(reason) = reason {
                self.walked.skipped.push(Skipped { path: path.clone(), reason });
                self.nodes.insert(path.as_str().to_owned(), Node::Other);
                return Ok(());
            }
        }
        for (path, kind, id) in children {
            self.child(path, kind, id)?;
        }
        Ok(())
    }

    fn child(&mut self, path: ContentPath, kind: EntryKind, id: ObjectId) -> Result<(), Refused> {
        let key = path.as_str().to_owned();
        match kind {
            EntryKind::Tree if self.source.output.contains(&path) => {
                self.nodes.insert(key, Node::Other);
                self.walked.skipped.push(Skipped { path, reason: Skip::Output });
            }
            EntryKind::Tree => {
                self.nodes.insert(key, Node::Folder);
                self.tree(id, Some(&path))?;
            }
            EntryKind::Commit => {
                self.nodes.insert(key, Node::Other);
                self.walked.skipped.push(Skipped { path, reason: Skip::Submodule });
            }
            EntryKind::Link => {
                self.count()?;
                let target = self.repo.find_blob(id).map_err(|source| Refused::Git { source })?.take_data();
                self.nodes.insert(key, Node::Link(target.clone().into()));
                self.links.push((path, target.into()));
            }
            EntryKind::Blob | EntryKind::BlobExecutable => {
                self.count()?;
                self.nodes.insert(key, Node::File(id));
                let (blob, size) = self.blob(id, &path)?;
                self.walked.entries.push(Entry::new(path, blob, size));
            }
        }
        Ok(())
    }

    /// One more file met, refused past the budget.
    fn count(&mut self) -> Result<(), Refused> {
        self.files += 1;
        let limit = self.source.budget.files;
        if self.files > limit {
            return Err(Refused::Files { count: self.files, limit });
        }
        Ok(())
    }

    /// The blob `id`, stored once however many paths name it. Its size is asked of the object
    /// store first, so a blob past the budget is never inflated.
    fn blob(&mut self, id: ObjectId, path: &ContentPath) -> Result<(Hash, Bytes), Refused> {
        if let Some(&known) = self.seen.get(&id) {
            return Ok(known);
        }
        let budget = self.source.budget;
        let git = |source| Refused::Git { source };
        let size = Bytes(self.repo.find_header(id).map_err(git)?.size());
        if size > budget.file {
            return Err(Refused::File { path: path.clone(), size, limit: budget.file });
        }
        self.total = self.total.saturating_add(size.count());
        if self.total > budget.total.count() {
            return Err(Refused::Total { size: Bytes(self.total), limit: budget.total });
        }
        let bytes = self.repo.find_blob(id).map_err(git)?.take_data();
        if Pointer(&bytes).is_lfs() {
            self.walked.pointers.push(path.clone());
        }
        let blob = self.store.put(bytes).map_err(Refused::Store)?;
        self.seen.insert(id, (blob, size));
        Ok((blob, size))
    }

    /// Whether a tree's `children` hold a file `name` starting with `mark`: a manifest `ink
    /// build` wrote ([`Folder::MANIFEST`]), or the mark of ink's cache ([`Disk::LABEL`]).
    fn holds(&self, children: &[(ContentPath, EntryKind, ObjectId)], name: &str, mark: &str) -> Result<bool, Refused> {
        let found = children.iter().find(|(path, kind, _)| {
            path.file_name() == name && matches!(kind, EntryKind::Blob | EntryKind::BlobExecutable)
        });
        let Some((_, _, id)) = found else { return Ok(false) };
        let bytes = self.repo.find_blob(*id).map_err(|source| Refused::Git { source })?;
        Ok(bytes.data.starts_with(mark.as_bytes()))
    }
}

/// One name of a tree, as git holds it.
#[derive(Clone, Copy, Debug)]
struct Name<'a> {
    prefix: Option<&'a ContentPath>,
    name: &'a BStr,
}

impl Name<'_> {
    /// The name as a path segment: UTF-8, and a name a site can hold.
    fn parse(self) -> Result<ContentPath, Refused> {
        let Ok(text) = self.name.to_str() else {
            let own = PathBuf::from(OsStr::from_bytes(self.name));
            let path = self.prefix.map_or_else(|| own.clone(), |prefix| PathBuf::from(prefix.as_str()).join(&own));
            return Err(Refused::Encoding { path });
        };
        ContentPath::parse(text).map_err(|source| Refused::Name { written: text.to_owned(), source })
    }
}

/// A link's text, as the path it names.
#[derive(Clone, Copy, Debug)]
struct Text<'a>(&'a BString);

impl From<Text<'_>> for PathBuf {
    fn from(Text(text): Text<'_>) -> Self {
        PathBuf::from(OsStr::from_bytes(text))
    }
}
