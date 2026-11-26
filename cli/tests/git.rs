//! `ink` on a git repository: the notes site committed over a few days, read at a commit with
//! its dates from the history. Each repository is written object by object with fixed names and
//! times, so its commit ids are the same on every run; `ink explain` is compared with
//! `fixture/git/<name>.snap`.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "tests stop at the first surprise"
)]

extern crate uses_ink_cli as cli;

use std::path::{Path, PathBuf};

use clap::Parser;
use cli::{Cli, Exit};
use gix::ObjectId;
use gix::objs::tree::EntryKind;

/// 2024-01-01T00:00:00Z: commit `n` is made `n` days later.
const EPOCH: i64 = 1_704_067_200;

/// `ink <args>`: the exit, stdout and stderr.
fn ink(args: &[&str]) -> (Exit, String, String) {
    let cli = Cli::try_parse_from(std::iter::once("ink").chain(args.iter().copied())).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = cli.run(&mut out, &mut err);
    (exit, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// A repository written object by object, every commit by Ann at a fixed time.
struct Repo {
    root: PathBuf,
    repo: gix::Repository,
}

impl Repo {
    fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-git").join(name);
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        let repo = gix::init(&root).unwrap();
        Self { root, repo }
    }

    /// Commits a tree holding exactly `files` on `parents`, on day `day`, as `main`.
    fn commit(&self, day: i64, parents: &[ObjectId], files: &[(&str, &str)]) -> ObjectId {
        let mut editor = self.repo.edit_tree(ObjectId::empty_tree(gix::hash::Kind::Sha1)).unwrap();
        for (path, text) in files {
            editor.upsert(*path, EntryKind::Blob, self.repo.write_blob(text).unwrap().detach()).unwrap();
        }
        let tree = editor.write().unwrap().detach();
        let time = gix::date::Time { seconds: EPOCH + day * 86_400, offset: 0 };
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
        std::fs::write(self.root.join(".git/refs/heads/main"), format!("{id}\n")).unwrap();
        std::fs::write(self.root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        id
    }

    fn dir(&self) -> &str {
        self.root.to_str().unwrap()
    }
}

const TEA: &str = "# Tea\n\nWater just off the boil. See [kettle](kettle.md).\n";
const TEA_AGAIN: &str = "# Tea\n\nWater just off the boil, for green tea a little less. See [kettle](kettle.md).\n";
const KETTLE: &str = "# Kettle\n\nDescale it with vinegar. Related: [tea](tea.md).\n";

/// The notes site, written on day 0, `kettle.md` moved into `notes/` on day 2 and `tea.md`
/// changed on day 3; the ids of the three commits.
fn notes(name: &str) -> (Repo, [ObjectId; 3]) {
    let repo = Repo::new(name);
    let one = repo.commit(0, &[], &[("notes/tea.md", TEA), ("kettle.md", KETTLE)]);
    let two = repo.commit(2, &[one], &[("notes/tea.md", TEA), ("notes/kettle.md", KETTLE)]);
    let three = repo.commit(3, &[two], &[("notes/tea.md", TEA_AGAIN), ("notes/kettle.md", KETTLE)]);
    (repo, [one, two, three])
}

#[test]
fn explain_shows_the_commit_and_dates_from_the_history() {
    let (full, _) = notes("full");
    let (shallow, [_, two, _]) = notes("shallow");
    std::fs::write(shallow.root.join(".git/shallow"), format!("{two}\n")).unwrap();
    let cases = [("full", &full), ("shallow", &shallow)].map(|(name, repo)| {
        let (exit, lines, stderr) = ink(&["explain", "-C", repo.dir()]);
        assert_eq!(exit, Exit::Done, "{stderr}");
        (name, lines)
    });
    fixture::cases("fixture/git", cases);
}

#[test]
fn a_repository_builds_the_same_deploy_twice_and_says_it_read_a_commit() {
    let (repo, [.., three]) = notes("twice");
    // the repository has no checkout: the only file on disk is one no commit holds
    std::fs::create_dir_all(repo.root.join("notes")).unwrap();
    std::fs::write(repo.root.join("notes/uncommitted.md"), "# Not in any commit\n").unwrap();
    let out = |at: &str| {
        let out = repo.root.with_file_name(format!("twice-{at}"));
        if out.exists() {
            std::fs::remove_dir_all(&out).unwrap();
        }
        out.to_str().unwrap().to_owned()
    };
    let (first, second) = (out("a"), out("b"));
    let (exit, deploy, stderr) = ink(&["build", repo.dir(), "-o", &first, "--base", "https://ink.example", "-v"]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert!(
        stderr.contains(&format!(
            "note[source.git.chosen]: the folder is a git repository, so the site is read from its commit {three}"
        )),
        "{stderr}"
    );
    assert!(stderr.contains("note[source.history.moved]"), "{stderr}");
    assert!(!Path::new(&first).join("notes/uncommitted/index.html").exists(), "the working copy is not read");
    let (exit, again, stderr) = ink(&["build", repo.dir(), "-o", &second, "--base", "https://ink.example"]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert_eq!(deploy, again);
    let (exit, folder, _) =
        ink(&["build", repo.dir(), "-o", &second, "--base", "https://ink.example", "--source", "folder"]);
    assert_eq!(exit, Exit::Done);
    assert_ne!(folder, deploy, "the files on disk hold one more page");
}

#[test]
fn rev_reads_an_older_commit_and_needs_a_repository() {
    let (repo, [one, ..]) = notes("rev");
    let (exit, lines, stderr) = ink(&["explain", "-C", repo.dir(), "--rev", "HEAD~2"]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    assert!(lines.contains(&format!("git commit {one}")), "{lines}");
    assert!(lines.contains("kettle.md\n"), "kettle.md was at the top then: {lines}");
    let (exit, _, stderr) = ink(&["check", repo.dir(), "--rev", "no-such-branch"]);
    assert_eq!(exit, Exit::Usage);
    assert!(stderr.contains("no-such-branch"), "{stderr}");
    let (exit, _, stderr) = ink(&["check", repo.dir(), "--rev", "HEAD", "--source", "folder"]);
    assert_eq!(exit, Exit::Usage);
    assert!(stderr.contains("`--rev` names a commit"), "{stderr}");
}
