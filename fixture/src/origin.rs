//! A repository of three commits for the tests that fetch: written object by object, with fixed
//! names and times, in a folder the test names. Behind the `origin` feature, since it takes gix
//! and a crate that runs on wasm does not need it.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

use std::path::{Path, PathBuf};

use gix::ObjectId;
use gix::objs::tree::EntryKind;

/// A repository of three commits on `main`, written object by object, every one by Ann at a fixed
/// time: `a.md` from the first day, changed on the second, `b.md` added on the third.
#[derive(Debug)]
pub struct Origin {
    /// The repository's folder.
    pub root: PathBuf,
    /// A folder beside it for a clone of it.
    pub cache: PathBuf,
    repo: gix::Repository,
}

impl Origin {
    const EPOCH: i64 = 1_704_067_200;

    /// The repository written under `base`, which is emptied first.
    ///
    /// # Panics
    ///
    /// When the folder cannot be written: a test has nothing to do then.
    pub fn new(base: impl AsRef<Path>) -> Self {
        let base = base.as_ref();
        if base.exists() {
            std::fs::remove_dir_all(base).unwrap();
        }
        let root = base.join("origin");
        std::fs::create_dir_all(&root).unwrap();
        let repo = gix::init(&root).unwrap();
        let origin = Self { root, cache: base.join("cache"), repo };
        let one = origin.commit(0, &[], &[("a.md", "# A\n")]);
        let two = origin.commit(1, &[one], &[("a.md", "# A, again\n")]);
        origin.commit(2, &[two], &[("a.md", "# A, again\n"), ("b.md", "# B\n")]);
        origin
    }

    fn commit(&self, day: i64, parents: &[ObjectId], files: &[(&str, &str)]) -> ObjectId {
        let repo = &self.repo;
        let root = &self.root;
        let mut editor = repo.edit_tree(ObjectId::empty_tree(gix::hash::Kind::Sha1)).unwrap();
        for (path, text) in files {
            editor.upsert(*path, EntryKind::Blob, repo.write_blob(text).unwrap().detach()).unwrap();
        }
        let tree = editor.write().unwrap().detach();
        let time = gix::date::Time { seconds: Self::EPOCH + day * 86_400, offset: 0 };
        let ann = gix::actor::Signature { name: "Ann".into(), email: "ann@ink.example".into(), time };
        let commit = gix::objs::Commit {
            tree,
            parents: parents.iter().copied().collect(),
            author: ann.clone(),
            committer: ann,
            encoding: None,
            message: format!("day {day}\n").into(),
            extra_headers: Vec::new(),
        };
        let id = repo.write_object(&commit).unwrap().detach();
        std::fs::write(root.join(".git/refs/heads/main"), format!("{id}\n")).unwrap();
        std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        id
    }

    /// The repository as a `file://` locator.
    pub fn url(&self) -> String {
        format!("file://{}", self.root.display())
    }
}
