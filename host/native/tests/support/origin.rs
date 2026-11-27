//! A repository of three commits for the tests that fetch: written object by object, with fixed
//! names and times, in a folder under the target directory. Included by each test target that
//! needs one, so the repository is built one way.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]
#![allow(dead_code, reason = "each test target uses the part it needs")]

use std::path::{Path, PathBuf};

use gix::ObjectId;
use gix::objs::tree::EntryKind;
use host_native::Fetch;
use model::git::{Depth, Locator, Network, Request};

/// A repository of three commits on `main`, written object by object, every one by Ann at a fixed
/// time: `a.md` from the first day, changed on the second, `b.md` added on the third.
pub(crate) struct Origin {
    pub(crate) root: PathBuf,
    pub(crate) cache: PathBuf,
    repo: gix::Repository,
}

impl Origin {
    const EPOCH: i64 = 1_704_067_200;

    pub(crate) fn new(name: &str) -> Self {
        let base = Path::new(env!("CARGO_TARGET_TMPDIR")).join("host-native-fetch").join(name);
        if base.exists() {
            std::fs::remove_dir_all(&base).unwrap();
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

    pub(crate) fn locator(&self) -> Locator {
        format!("file://{}", self.root.display()).parse().unwrap()
    }

    pub(crate) fn fetch(&self) -> Fetch {
        Fetch { network: Network::Local, ..Fetch::new(&self.cache) }
    }

    pub(crate) fn request(&self, depth: Depth) -> Request {
        let mut request = Request::new(self.locator());
        request.depth = depth;
        request
    }
}
