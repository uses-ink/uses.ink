//! A remote's branch and tag, read from its reference list with no object fetched, through the
//! file transport.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise")]

extern crate uses_ink_watch_git as watch_git;

use std::path::Path;

use build::Revision;
use gix::ObjectId;
use gix::objs::tree::EntryKind;
use model::git::{Locator, Network, Scheme};
use watch::{Head, Heads};
use watch_git::{Missing, Remotes, Unlisted};

/// A repository written object by object, with a `main` branch.
struct Repo {
    root: std::path::PathBuf,
    repo: gix::Repository,
}

impl Repo {
    fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("watch-git").join(name);
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        let repo = gix::init(&root).unwrap();
        Self { root, repo }
    }

    fn commit(&self, day: i64, parents: &[ObjectId], branch: &str) -> ObjectId {
        let mut editor = self.repo.edit_tree(ObjectId::empty_tree(gix::hash::Kind::Sha1)).unwrap();
        editor
            .upsert("index.md", EntryKind::Blob, self.repo.write_blob(format!("# day {day}\n")).unwrap().detach())
            .unwrap();
        let tree = editor.write().unwrap().detach();
        let time = gix::date::Time { seconds: 1_704_067_200 + day * 86_400, offset: 0 };
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
        let id = self.repo.write_object(&commit).unwrap().detach();
        std::fs::write(self.root.join(".git/refs/heads").join(branch), format!("{id}\n")).unwrap();
        std::fs::write(self.root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        id
    }

    fn locator(&self) -> Locator {
        format!("file://{}", self.root.display()).parse().unwrap()
    }
}

fn head(id: ObjectId) -> Head {
    id.to_hex().to_string().parse().unwrap()
}

#[test]
fn reads_the_default_branch_a_named_branch_and_a_moved_head() {
    let repo = Repo::new("heads");
    let first = repo.commit(0, &[], "main");
    let draft = repo.commit(1, &[first], "draft");
    let scratch = tempfile::tempdir().unwrap();
    let remotes = Remotes::new(scratch.path().join("heads"), Network::Local);
    let at = |revision: Option<&str>| {
        let revision = revision.map(|text| text.parse::<Revision>().unwrap());
        tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(remotes.head(
            &repo.locator(),
            revision.as_ref(),
            None,
        ))
    };
    assert_eq!(at(None).unwrap(), head(first));
    assert_eq!(at(Some("draft")).unwrap(), head(draft));
    let missing = at(Some("nothing")).unwrap_err();
    assert!(matches!(missing, Unlisted::Missing(Missing::Revision { .. })), "{missing:?}");
    let second = repo.commit(2, &[first], "main");
    assert_eq!(at(None).unwrap(), head(second), "a moved branch is a new head");
    assert!(scratch.path().join("heads").join("HEAD").is_file(), "the connection ran in the kept repository");
}

#[test]
fn the_network_setting_says_no_before_anything_is_asked() {
    let repo = Repo::new("refused");
    repo.commit(0, &[], "main");
    let scratch = tempfile::tempdir().unwrap();
    let ask = |network: Network, locator: &Locator| {
        let remotes = Remotes::new(scratch.path().join("refused"), network);
        tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(remotes.head(locator, None, None))
    };
    let off = ask(Network::Off, &repo.locator()).unwrap_err();
    assert!(matches!(off, Unlisted::Network { scheme: Scheme::File, .. }), "{off:?}");
    assert!(!scratch.path().join("refused").exists(), "nothing was made for a refused ask");
}
