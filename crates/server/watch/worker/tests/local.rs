//! A local repository read two ways: by `watch/git` through the file transport, and by this crate
//! from the advertisement `git upload-pack` makes of the same repository. Annotated tags, a
//! lightweight tag, branches and a detached feature branch all must read the same.

#![allow(clippy::unwrap_used, clippy::panic, clippy::too_many_lines, reason = "tests stop at the first surprise")]

extern crate uses_ink_watch_worker as watch_worker;

use std::path::Path;
use std::process::Command;

use build::Revision;
use model::git::{Locator, Network};
use watch::Heads;
use watch_git::Remotes;
use watch_worker::{Replay, Reply, Smart};

/// Runs `git` in `dir`, as a fixed author.
fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["-c", "init.defaultBranch=main", "-c", "user.name=Ann", "-c", "user.email=ann@ink.example"])
        .args(args)
        .env("GIT_AUTHOR_DATE", "2024-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2024-01-01T00:00:00Z")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

/// The advertisement `git http-backend` would send: the service packet, a flush, and what
/// `upload-pack` lists.
fn advertisement(dir: &Path) -> Vec<u8> {
    let out =
        Command::new("git").args(["upload-pack", "--advertise-refs", "--stateless-rpc"]).arg(dir).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let mut body = b"001e# service=git-upload-pack\n0000".to_vec();
    body.extend(out.stdout);
    body
}

#[test]
fn both_readers_name_the_same_commits() {
    let repo = tempfile::tempdir().unwrap();
    let dir = repo.path();
    git(dir, &["init", "-q"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", "one"]);
    git(dir, &["tag", "light"]);
    git(dir, &["tag", "-a", "annotated", "-m", "a tag object"]);
    git(dir, &["checkout", "-q", "-b", "draft"]);
    git(dir, &["commit", "-q", "--allow-empty", "-m", "two"]);
    git(dir, &["checkout", "-q", "main"]);

    let url = "https://forge.example/ink/site.git";
    let locator: Locator = url.parse().unwrap();
    let worker = Smart::new(
        Replay::new().serving(
            format!("{url}/info/refs?service=git-upload-pack"),
            Reply { status: 200, body: advertisement(dir) },
        ),
        Network::On,
    );
    let scratch = tempfile::tempdir().unwrap();
    let local = Remotes::new(scratch.path().join("heads"), Network::Local);
    let file: Locator = format!("file://{}", dir.display()).parse().unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();

    let ask = |revision: Option<&str>| {
        let revision = revision.map(|text| text.parse::<Revision>().unwrap());
        let both = (
            pollster::block_on(worker.head(&locator, revision.as_ref(), None)).unwrap(),
            runtime.block_on(local.head(&file, revision.as_ref(), None)).unwrap(),
        );
        assert_eq!(both.0, both.1, "{revision:?}");
        both.0
    };
    let main = ask(None);
    assert_eq!(main.as_str(), git(dir, &["rev-parse", "main"]).trim());
    assert_eq!(ask(Some("draft")).as_str(), git(dir, &["rev-parse", "draft"]).trim());
    assert_eq!(ask(Some("light")), main);
    assert_eq!(ask(Some("annotated")).as_str(), git(dir, &["rev-parse", "annotated^{commit}"]).trim());
    assert_eq!(ask(Some(main.as_str())), main, "a commit id names itself");

    let (theirs, ours) = (
        pollster::block_on(worker.branches(&locator, None)).unwrap(),
        runtime.block_on(local.branches(&file, None)).unwrap(),
    );
    assert_eq!(theirs, ours);
    assert_eq!(theirs.default.map(|branch| branch.to_string()), Some("main".to_owned()));
}
