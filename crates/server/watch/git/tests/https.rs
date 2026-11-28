//! A remote's head read over https from a local TLS server whose certificate authority is made
//! here (decision 0072): no network is touched, and `git` must be on the path.

#![allow(clippy::unwrap_used, clippy::panic, reason = "tests stop at the first surprise")]

extern crate uses_ink_watch_git as watch_git;

#[path = "../../../../core/https/tests/support/server.rs"]
mod server;

use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use https::Https;
use model::git::{Locator, Network};
use server::{Authority, Mode, Plain, Server};
use watch::Heads;
use watch_git::{Remotes, Unlisted};

/// A repository of one commit on `main`, made by `git`.
struct Origin {
    root: PathBuf,
    scratch: tempfile::TempDir,
}

impl Origin {
    fn new(name: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("watch-git-https").join(name);
        if root.exists() {
            std::fs::remove_dir_all(&root).unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        for args in [["init", "--quiet", "-b", "main"], ["commit", "--quiet", "--allow-empty", "-m"]] {
            let mut command = Command::new("git");
            command.current_dir(&root).args(args);
            if args[0] == "commit" {
                command.arg("day 0");
            }
            let status = command
                .env("GIT_AUTHOR_NAME", "Ann")
                .env("GIT_AUTHOR_EMAIL", "ann@ink.example")
                .env("GIT_COMMITTER_NAME", "Ann")
                .env("GIT_COMMITTER_EMAIL", "ann@ink.example")
                .status()
                .unwrap();
            assert!(status.success());
        }
        Self { root, scratch: tempfile::tempdir().unwrap() }
    }

    fn commit(&self) -> String {
        let out = Command::new("git").current_dir(&self.root).args(["rev-parse", "HEAD"]).output().unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    }

    fn remotes(&self, https: Https) -> Remotes {
        Remotes::new(self.scratch.path().join("heads"), Network::On).with_https(https)
    }
}

fn head(remotes: &Remotes, url: &str) -> Result<watch::Head, Unlisted> {
    let locator: Locator = url.parse().unwrap();
    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(remotes.head(&locator, None, None))
}

/// Every message in an error's chain of causes, so a test can name the cause it expects.
fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut next = error.source();
    while let Some(cause) = next {
        write!(text, " | {cause} {cause:?}").unwrap();
        next = cause.source();
    }
    text
}

#[test]
fn a_head_is_read_over_https_from_a_root_the_client_trusts() {
    let origin = Origin::new("trusted");
    let authority = Authority::new();
    let server = Server::start(&authority, &origin.root, Mode::Serve);
    let remotes = origin.remotes(Https::default().trusting(authority.root()));
    let read = head(&remotes, &server.url()).unwrap();
    assert_eq!(read, origin.commit().parse().unwrap());
    assert!(server.requests() >= 1);
}

#[test]
fn a_certificate_no_root_vouches_for_is_a_typed_failure_and_nothing_falls_back() {
    let origin = Origin::new("untrusted");
    let server = Server::start(&Authority::new(), &origin.root, Mode::Serve);
    let stranger = Authority::new();
    let remotes = origin.remotes(Https::default().trusting(stranger.root()));
    let refused = head(&remotes, &server.url()).unwrap_err();
    assert!(matches!(refused, Unlisted::Remote { .. }), "{refused:?}");
    let text = chain(&refused);
    assert!(text.contains("UnknownIssuer") || text.to_lowercase().contains("certificate"), "{text}");
    assert_eq!(server.requests(), 0, "no request was sent over a connection that failed verification");

    let none = head(&origin.remotes(Https::default()), &server.url()).unwrap_err();
    assert!(matches!(none, Unlisted::Remote { .. }), "the bundled roots alone do not trust it: {none:?}");
}

#[test]
fn a_redirect_to_plain_http_is_not_followed() {
    let origin = Origin::new("downgrade");
    let authority = Authority::new();
    let plain = Plain::start();
    let to = format!("{}/info/refs?service=git-upload-pack", plain.url());
    let server = Server::start(&authority, &origin.root, Mode::Redirect(to));
    let remotes = origin.remotes(Https::default().trusting(authority.root()));
    let refused = head(&remotes, &server.url()).unwrap_err();
    assert!(matches!(refused, Unlisted::Remote { .. }), "{refused:?}");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(plain.accepted(), 0, "the plain listener was never connected to: {}", chain(&refused));
}

#[test]
fn a_server_that_never_answers_is_cut_at_the_total_timeout() {
    let origin = Origin::new("stall");
    let authority = Authority::new();
    let server = Server::start(&authority, &origin.root, Mode::Stall);
    let https = Https { total: Duration::from_millis(500), ..Https::default() }.trusting(authority.root());
    let started = Instant::now();
    let refused = head(&origin.remotes(https), &server.url()).unwrap_err();
    assert!(matches!(refused, Unlisted::Remote { .. }), "{refused:?}");
    assert!(started.elapsed() < Duration::from_secs(10), "took {:?}", started.elapsed());
    assert!(
        chain(&refused).to_lowercase().contains("timed out") || chain(&refused).to_lowercase().contains("timeout"),
        "{}",
        chain(&refused)
    );
}
