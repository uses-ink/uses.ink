//! The snapshot as a site's files: where the content starts (`site.root`), the files under it
//! and the rest, and how they are arranged (`site.layout`).

mod layout;
mod root;

use std::collections::BTreeSet;

use model::{ContentPath, Entry, FileFacts, Snapshot};

pub(crate) use layout::Layout;
pub(crate) use root::Root;

use crate::interpret::decide::Filed;
use crate::interpret::decide::witness::Repository;

/// The snapshot as `site.root` sees it: its files and which of them a format reads.
#[derive(Debug)]
pub(crate) struct Tree<'a> {
    snapshot: &'a Snapshot,
    /// The files a format reads.
    readable: BTreeSet<&'a ContentPath>,
}

impl Tree<'_> {
    /// Folders that hold a site's content when they are the only place content is. Not `posts`
    /// or `blog`: a root there would make every post a plain page, as `page.kind` reads a posts
    /// folder below the root.
    const CONTENT: &[&str] = &["content", "docs", "notes", "src/content"];

    /// The one folder of [`Tree::CONTENT`] present, when every file a format reads is in it
    /// (repository files such as `README.md` and `LICENSE` at the top aside).
    pub(crate) fn content(&self) -> Option<ContentPath> {
        let tree = self.snapshot.tree();
        let present: Vec<ContentPath> = Self::CONTENT
            .iter()
            .filter_map(|folder| ContentPath::parse(*folder).ok())
            .filter(|folder| tree.iter().any(|entry| entry.path.strip(folder).is_some()))
            .collect();
        let [only] = present.as_slice() else { return None };
        let stray = tree.iter().any(|entry| {
            entry.path.strip(only).is_none() && self.readable.contains(&entry.path) && !Repository::holds(&entry.path)
        });
        (!stray).then(|| only.clone())
    }
}

/// A file under the root.
#[derive(Clone, Debug)]
pub(crate) struct InRoot<'a> {
    pub(crate) entry: &'a Entry,
    /// Its path below the root: `posts/a.md` under root `content` for `content/posts/a.md`.
    pub(crate) relative: ContentPath,
    /// What the source knows about it beyond its bytes.
    pub(crate) facts: Option<&'a FileFacts>,
}

impl Filed for InRoot<'_> {
    fn path(&self) -> &ContentPath {
        &self.entry.path
    }

    fn relative(&self) -> &ContentPath {
        &self.relative
    }

    fn facts(&self) -> Option<&FileFacts> {
        self.facts
    }
}

/// The snapshot split at the root.
#[derive(Debug)]
pub(crate) struct Scope<'a> {
    pub(crate) root: model::Root,
    /// In path order.
    pub(crate) inside: Vec<InRoot<'a>>,
    /// In path order.
    pub(crate) outside: Vec<&'a Entry>,
    pub(crate) snapshot: &'a Snapshot,
}

impl<'a> Scope<'a> {
    /// `snapshot` split at `root`.
    pub(crate) fn new(snapshot: &'a Snapshot, root: model::Root) -> Self {
        let mut inside = Vec::new();
        let mut outside = Vec::new();
        for entry in snapshot.tree() {
            let relative = match &root {
                model::Root::Folder(folder) => entry.path.strip(folder),
                _ => Some(entry.path.clone()),
            };
            match relative {
                Some(relative) => inside.push(InRoot { entry, relative, facts: snapshot.facts_of(&entry.path) }),
                None => outside.push(entry),
            }
        }
        Self { root, inside, outside, snapshot }
    }

    /// The root's folder, or `None` at the top of the snapshot.
    pub(crate) fn folder(&self) -> Option<&ContentPath> {
        match &self.root {
            model::Root::Folder(folder) => Some(folder),
            _ => None,
        }
    }

    /// The snapshot path of `relative`, a path below the root.
    pub(crate) fn absolute(&self, relative: &ContentPath) -> ContentPath {
        self.folder().map_or_else(|| relative.clone(), |folder| folder.join(relative))
    }
}
