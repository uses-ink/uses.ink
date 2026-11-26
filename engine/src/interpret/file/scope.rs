//! The files under the site's root, each with its path below the root, and those outside it.

use model::{ContentPath, Entry, FileFacts, Root, Snapshot, Subject};

/// A file under the root.
#[derive(Clone, Debug)]
pub(crate) struct InRoot<'a> {
    pub(crate) entry: &'a Entry,
    /// Its path below the root: `posts/a.md` under root `content` for `content/posts/a.md`.
    pub(crate) relative: ContentPath,
    /// What the source knows about it beyond its bytes.
    pub(crate) facts: Option<&'a FileFacts>,
}

impl InRoot<'_> {
    pub(crate) fn path(&self) -> &ContentPath {
        &self.entry.path
    }

    pub(crate) fn subject(&self) -> Subject {
        Subject::File(self.entry.path.clone())
    }
}

/// The snapshot split at the root.
#[derive(Debug)]
pub(crate) struct Scope<'a> {
    pub(crate) root: Root,
    /// In path order.
    pub(crate) inside: Vec<InRoot<'a>>,
    /// In path order.
    pub(crate) outside: Vec<&'a Entry>,
}

impl<'a> Scope<'a> {
    pub(crate) fn new(snapshot: &'a Snapshot, root: Root) -> Self {
        let mut inside = Vec::new();
        let mut outside = Vec::new();
        for entry in snapshot.tree() {
            let relative = match &root {
                Root::Folder(folder) => entry.path.strip(folder),
                _ => Some(entry.path.clone()),
            };
            match relative {
                Some(relative) => inside.push(InRoot { entry, relative, facts: snapshot.facts_of(&entry.path) }),
                None => outside.push(entry),
            }
        }
        Self { root, inside, outside }
    }

    /// The snapshot path of `relative`, a path below the root.
    pub(crate) fn absolute(&self, relative: &ContentPath) -> ContentPath {
        match &self.root {
            Root::Folder(folder) => folder.join(relative),
            _ => relative.clone(),
        }
    }

    /// The root's folder, or `None` at the top of the snapshot.
    pub(crate) fn absolute_folder(&self) -> Option<ContentPath> {
        match &self.root {
            Root::Folder(folder) => Some(folder.clone()),
            _ => None,
        }
    }
}
