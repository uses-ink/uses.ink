//! Git repositories read through the public API. Each repository is built here, object by
//! object, with fixed names and times, so every commit id is the same on every run; the
//! snapshots are compared with `fixture/git/<name>.snap`.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

extern crate uses_ink_host_native as host_native;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use gix::ObjectId;
use gix::objs::tree::EntryKind;
use host_native::{Budget, GitLimits, Memory, Read, Refused, Repository, Revision};
use model::{Diagnostic, Known, Provenance, Timestamp, Version};

/// What a path of a commit's tree holds.
#[derive(Clone, Copy, Debug)]
enum Node<'a> {
    Text(&'a str),
    Link(&'a str),
    Submodule,
}

/// A repository written object by object, every commit by Ann at a fixed time.
#[derive(Debug)]
struct Repo {
    root: PathBuf,
    repo: gix::Repository,
}

impl Repo {
    /// 2024-01-01T00:00:00Z: commit `n` is made `n` days later.
    const EPOCH: i64 = 1_704_067_200;

    /// A fresh, empty repository named for the test.
    fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("host-native-git").join(name);
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        let repo = gix::init(&root).unwrap();
        Self { root, repo }
    }

    /// Commits a tree holding exactly `files` on `parents`, on day `day`, and points `main` at it.
    fn commit(&self, day: i64, parents: &[ObjectId], files: &[(&str, Node<'_>)]) -> ObjectId {
        let empty = ObjectId::empty_tree(gix::hash::Kind::Sha1);
        let mut editor = self.repo.edit_tree(empty).unwrap();
        for (path, node) in files {
            let (kind, id) = match node {
                Node::Text(text) => (EntryKind::Blob, self.repo.write_blob(text).unwrap().detach()),
                Node::Link(target) => (EntryKind::Link, self.repo.write_blob(target).unwrap().detach()),
                // a submodule's commit lives in another repository: any id will do
                Node::Submodule => {
                    (EntryKind::Commit, ObjectId::from_hex(b"1111111111111111111111111111111111111111").unwrap())
                }
            };
            editor.upsert(*path, kind, id).unwrap();
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
        let id = self.repo.write_object(&commit).unwrap().detach();
        std::fs::write(self.root.join(".git/refs/heads/main"), format!("{id}\n")).unwrap();
        std::fs::write(self.root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        id
    }

    /// Makes the repository shallow at `boundary`, as a clone of that depth would be: its
    /// parents stay in the object store, and the history stops there.
    fn shallow(&self, boundary: ObjectId) {
        std::fs::write(self.root.join(".git/shallow"), format!("{boundary}\n")).unwrap();
    }

    fn read(&self) -> Result<Read, Refused> {
        Repository::new(&self.root).read(&mut Memory::default())
    }
}

/// The history every snapshot test reads: a file changed, a file moved, a file deleted and added
/// again, a branch merged, a link to a file and one to a folder, a submodule and an LFS pointer.
fn history(name: &str) -> (Repo, Vec<ObjectId>) {
    let repo = Repo::new(name);
    let pointer = "version https://git-lfs.github.com/spec/v1\noid sha256:4d7a\nsize 12\n";
    let one = repo.commit(
        0,
        &[],
        &[("a.md", Node::Text("# A\n")), ("old.md", Node::Text("moved\n")), ("gone.md", Node::Text("first\n"))],
    );
    let two = repo.commit(1, &[one], &[("a.md", Node::Text("# A\n")), ("old.md", Node::Text("moved\n"))]);
    let side = repo.commit(
        2,
        &[two],
        &[("a.md", Node::Text("# A\n")), ("old.md", Node::Text("moved\n")), ("notes/side.md", Node::Text("side\n"))],
    );
    let three = repo.commit(3, &[two], &[("a.md", Node::Text("# A, again\n")), ("old.md", Node::Text("moved\n"))]);
    let merged = repo.commit(
        4,
        &[three, side],
        &[
            ("a.md", Node::Text("# A, again\n")),
            ("old.md", Node::Text("moved\n")),
            ("notes/side.md", Node::Text("side\n")),
        ],
    );
    let five = repo.commit(
        5,
        &[merged],
        &[
            ("a.md", Node::Text("# A, again\n")),
            ("new.md", Node::Text("moved\n")),
            ("notes/side.md", Node::Text("side\n")),
            ("gone.md", Node::Text("back\n")),
            ("big.bin", Node::Text(pointer)),
            ("link.md", Node::Link("a.md")),
            ("notes/up.md", Node::Link("../new.md")),
            ("alias", Node::Link("notes")),
            ("vendor/lib", Node::Submodule),
        ],
    );
    (repo, vec![one, two, side, three, merged, five])
}

/// A snapshot as lines: its version, what it had and lacked, each file with its facts, then
/// what was skipped and kept as a pointer.
fn render(read: &Read) -> String {
    let snapshot = &read.snapshot;
    let mut out = String::new();
    let _infallible = writeln!(out, "version {:?}", snapshot.version());
    let _infallible = writeln!(out, "has {:?}", snapshot.observed().has);
    for lack in &snapshot.observed().lacks {
        let _infallible = writeln!(out, "lacks {:?} for {} as {:?}", lack.ability, lack.consequence.name(), lack.gap);
    }
    for entry in snapshot.tree() {
        let facts = snapshot.facts_of(&entry.path).unwrap();
        let _infallible = writeln!(out, "{} ({})", entry.path, entry.size);
        let _infallible = writeln!(out, "  created {}", Fact(&facts.created));
        let _infallible = writeln!(out, "  updated {}", Fact(&facts.updated));
    }
    for skipped in &read.skipped {
        let _infallible = writeln!(out, "skipped {} as {:?}", skipped.path, skipped.reason);
    }
    for pointer in &read.pointers {
        let _infallible = writeln!(out, "pointer {pointer}");
    }
    for notice in read.notices() {
        let _infallible = writeln!(out, "notice {}", notice.code());
    }
    out
}

/// A fact, its commit shortened.
struct Fact<'a>(&'a Known<Timestamp>);

impl std::fmt::Display for Fact<'_> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let short = |from: &Provenance| match from {
            Provenance::Commit { commit } => format!("commit {}", &commit.as_str()[..10]),
            Provenance::GitHistory { commit } => format!("history up to {}", &commit.as_str()[..10]),
            other => format!("{other:?}"),
        };
        match self.0 {
            Known::Exact { value, from } => write!(out, "{value} from {}", short(from)),
            Known::NotAfter { value, from } => write!(out, "not after {value} from {}", short(from)),
            Known::Unknown { reason } => write!(out, "unknown: {reason:?}"),
            other => write!(out, "{other:?}"),
        }
    }
}

fn code(refused: &Refused) -> &'static str {
    Diagnostic::from(refused).code()
}

#[test]
fn snapshots_hold_the_commit_and_its_dates() {
    let (full, _) = history("full");
    let (shallow, commits) = history("shallow");
    shallow.shallow(commits[4]);
    fixture::cases(
        "fixture/git",
        [("full", render(&full.read().unwrap())), ("shallow", render(&shallow.read().unwrap()))],
    );
}

#[test]
fn the_version_is_the_commit_and_the_dates_name_theirs() {
    let (repo, commits) = history("version");
    let read = repo.read().unwrap();
    let Version::GitCommit(oid) = read.snapshot.version() else { panic!("{:?}", read.snapshot.version()) };
    assert_eq!(oid.as_str(), commits[5].to_string());
    let facts = read.snapshot.facts_of(&"a.md".parse().unwrap()).unwrap();
    let Known::Exact { from: Provenance::Commit { commit }, .. } = &facts.updated else { panic!("{facts:?}") };
    assert_eq!(commit.as_str(), commits[3].to_string(), "changed on day 3, not by the merge");
    assert_eq!(repo.read().unwrap(), read, "the same repository reads the same");
}

#[test]
fn reads_the_commit_a_revision_names_not_the_working_copy() {
    let (repo, commits) = history("revision");
    std::fs::write(repo.root.join("uncommitted.md"), "not in any commit").unwrap();
    let head = repo.read().unwrap();
    assert!(head.snapshot.entry(&"uncommitted.md".parse().unwrap()).is_none());
    let older = Repository { revision: Revision::Spec("HEAD~1".into()), ..Repository::new(&repo.root) };
    let read = older.read(&mut Memory::default()).unwrap();
    let Version::GitCommit(oid) = read.snapshot.version() else { panic!() };
    assert_eq!(oid.as_str(), commits[4].to_string());
    assert!(read.snapshot.entry(&"old.md".parse().unwrap()).is_some(), "before the move");
    let unknown = Repository { revision: Revision::Spec("no-such-branch".into()), ..Repository::new(&repo.root) };
    let refused = unknown.read(&mut Memory::default()).unwrap_err();
    assert!(matches!(&refused, Refused::Revision { written, .. } if written == "no-such-branch"), "{refused:?}");
    assert_eq!(code(&refused), "source.revision.unknown");
}

#[test]
fn refuses_links_out_of_the_tree_and_links_to_nothing() {
    let repo = Repo::new("links");
    repo.commit(0, &[], &[("a.md", Node::Text("a")), ("leak.md", Node::Link("../../etc/passwd"))]);
    let refused = repo.read().unwrap_err();
    assert!(matches!(&refused, Refused::Outside { path, .. } if path.as_str() == "leak.md"), "{refused:?}");
    assert_eq!(code(&refused), "source.link.outside");
    repo.commit(1, &[], &[("a.md", Node::Text("a")), ("dangling.md", Node::Link("missing.md"))]);
    let refused = repo.read().unwrap_err();
    assert!(matches!(&refused, Refused::Broken { path, .. } if path.as_str() == "dangling.md"), "{refused:?}");
    assert_eq!(code(&refused), "source.link.broken");
}

/// Slow by construction, and slower on a busy host, which is not a lock to fix. A tree 6,000 folders
/// deep is 6,000 loose objects: the fixture writes each as a temporary file renamed into a
/// fan-out folder, and the read opens each one again. Measured on an idle host this takes 1.1 s of
/// CPU in 1.1 s of wall time, of which 0.82 s is kernel time (0.31 s user): nothing in the read
/// waits on a lock, a cache or a process, and the walk is one thread. Beside builds that write to
/// the same filesystem those small file operations are what slows down, the likely source of the
/// 14 to 30 s seen on a loaded farm host (not reproduced on petal beside the crate's own tests,
/// where it takes 1.3 s). The depth is the point of the test
/// (it must pass the stack a recursive walk has), so it is not lowered; the CI profile gives this
/// binary its own budget (`.config/nextest.toml`).
#[test]
fn a_tree_nested_far_past_the_stack_is_read() {
    let repo = Repo::new("deep");
    let deep = format!("{}a.md", "d/".repeat(6_000));
    repo.commit(0, &[], &[(deep.as_str(), Node::Text("a"))]);
    let read = repo.read().unwrap();
    assert_eq!(read.snapshot.tree().len(), 1);
}

#[test]
fn refuses_past_each_budget_with_the_numbers() {
    let repo = Repo::new("budget");
    repo.commit(0, &[], &[("a.md", Node::Text("aaaa")), ("b.md", Node::Text("bbbb")), ("c.md", Node::Text("cccc"))]);
    let read = |budget| Repository { budget, ..Repository::new(&repo.root) }.read(&mut Memory::default());
    let refused = read(Budget { files: 2, ..Budget::default() }).unwrap_err();
    assert!(matches!(refused, Refused::Files { count: 3, limit: 2 }), "{refused:?}");
    let refused = read(Budget { file: model::Bytes(3), ..Budget::default() }).unwrap_err();
    assert!(matches!(&refused, Refused::File { path, .. } if path.as_str() == "a.md"), "{refused:?}");
    let refused = read(Budget { total: model::Bytes(10), ..Budget::default() }).unwrap_err();
    assert!(matches!(refused, Refused::Total { limit: model::Bytes(10), .. }), "{refused:?}");
    let refused = read(Budget { folders: 0, ..Budget::default() }).unwrap_err();
    assert_eq!(code(&refused), "source.budget.folders");
}

#[test]
fn skips_output_the_build_wrote_even_when_committed() {
    let repo = Repo::new("output");
    let mark = format!("{}v2\",\"files\":[]}}", host_native::Folder::MARK);
    repo.commit(
        0,
        &[],
        &[
            ("a.md", Node::Text("a")),
            ("site/.ink-manifest.json", Node::Text(&mark)),
            ("site/index.html", Node::Text("<p>")),
        ],
    );
    let read = repo.read().unwrap();
    let paths: Vec<&str> = read.snapshot.tree().iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(paths, ["a.md"]);
    assert_eq!(read.skipped[0].path.as_str(), "site");
}

#[test]
fn refuses_a_folder_that_is_no_repository() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("host-native-git/none");
    std::fs::create_dir_all(&root).unwrap();
    let refused = Repository::new(&root).read(&mut Memory::default()).unwrap_err();
    assert!(matches!(refused, Refused::Repository { .. }), "{refused:?}");
    assert_eq!(code(&refused), "source.repository.unreadable");
}

#[test]
fn refuses_a_chain_of_links_past_the_limit_with_the_limit() {
    let repo = Repo::new("hops");
    repo.commit(
        0,
        &[],
        &[
            ("a.md", Node::Text("a")),
            ("one.md", Node::Link("two.md")),
            ("two.md", Node::Link("three.md")),
            ("three.md", Node::Link("a.md")),
        ],
    );
    repo.read().unwrap();
    let limits = GitLimits { hops: 1, ..GitLimits::default() };
    let refused = Repository { limits, ..Repository::new(&repo.root) }.read(&mut Memory::default()).unwrap_err();
    assert!(matches!(refused, Refused::Looped { limit: 1, .. }), "{refused:?}");
    assert_eq!(code(&refused), "source.link.broken");
}

#[test]
fn the_snapshot_does_not_follow_the_threads_or_blocks_it_was_read_on() {
    let repo = Repo::new("threads");
    let first = repo.commit(0, &[], &[("a.md", Node::Text("a"))]);
    repo.commit(1, &[first], &[("a.md", Node::Text("b")), ("b.md", Node::Text("b"))]);
    let one = GitLimits {
        threads: std::num::NonZeroUsize::MIN,
        block: std::num::NonZeroUsize::MIN,
        deltas: std::num::NonZeroUsize::MIN,
        ..GitLimits::default()
    };
    let narrow = Repository { limits: one, ..Repository::new(&repo.root) }.read(&mut Memory::default()).unwrap();
    assert_eq!(narrow.snapshot, repo.read().unwrap().snapshot);
}
