//! The whole self-host loop through the real `ink` binary, with no network: `ink serve --data`
//! on a temp folder, a git repository on disk as the site, added through the API, served by
//! host, changed by a commit, rolled back, exported and deleted. The test waits on the loops'
//! events on stderr, never on a sleep.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "tests stop at the first surprise"
)]

use std::fmt::Write as _;
use std::io::{Read, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Receiver;
use std::time::Duration;

mod guard;
mod keyed;

use gix::ObjectId;
use gix::objs::tree::EntryKind;
use serde_json::{Value, json};

/// How long an event may take to come before the test gives up: a build of a small site, on a
/// loaded machine.
const PATIENCE: Duration = Duration::from_secs(60);

/// A repository written object by object, every commit by Ann.
struct Repo {
    root: PathBuf,
    objects: gix::Repository,
    day: i64,
}

impl Repo {
    fn new(root: &Path) -> Self {
        std::fs::create_dir_all(root).unwrap();
        Self { root: root.to_path_buf(), objects: gix::init(root).unwrap(), day: 0 }
    }

    /// Commits a tree of exactly `files` on `parents`, as `main`.
    fn commit(&mut self, parents: &[ObjectId], files: &[(&str, &str)]) -> ObjectId {
        let mut editor = self.objects.edit_tree(ObjectId::empty_tree(gix::hash::Kind::Sha1)).unwrap();
        for (path, text) in files {
            editor.upsert(*path, EntryKind::Blob, self.objects.write_blob(text).unwrap().detach()).unwrap();
        }
        let tree = editor.write().unwrap().detach();
        self.day += 1;
        let time = gix::date::Time { seconds: 1_704_067_200 + self.day * 86_400, offset: 0 };
        let ann = gix::actor::Signature { name: "Ann".into(), email: "ann@ink.example".into(), time };
        let commit = gix::objs::Commit {
            tree,
            parents: parents.iter().copied().collect(),
            author: ann.clone(),
            committer: ann,
            encoding: None,
            message: format!("day {}\n", self.day).into(),
            extra_headers: Vec::new(),
        };
        let id = self.objects.write_object(&commit).unwrap().detach();
        std::fs::write(self.root.join(".git/refs/heads/main"), format!("{id}\n")).unwrap();
        std::fs::write(self.root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        id
    }
}

/// Lines of the child's output, as they are printed.
struct Lines(Receiver<String>);

impl Lines {
    /// The next line that holds `needle`.
    fn until(&self, needle: &str) -> String {
        loop {
            let line = self.0.recv_timeout(PATIENCE).unwrap_or_else(|_| panic!("no line with `{needle}` came"));
            if line.contains(needle) {
                return line;
            }
        }
    }
}

/// The running `ink serve`, killed when it goes out of scope.
struct Running {
    lines: Lines,
    port: u16,
    key: keyed::Keyed,
    // last: the child is killed after the rest is gone, and a panic in `start` still drops it
    _child: guard::Guard,
}

/// What a request does.
#[derive(Clone, Copy, Debug)]
enum Verb {
    Get,
    Post,
    Delete,
}

impl Verb {
    fn name(self) -> String {
        format!("{self:?}").to_ascii_uppercase()
    }
}

/// One response.
struct Got {
    text: String,
}

impl Got {
    fn status(&self) -> &str {
        self.text.split(' ').nth(1).unwrap_or_default()
    }

    fn body(&self) -> &str {
        self.text.split_once("\r\n\r\n").map_or("", |(_, body)| body)
    }

    fn json(&self) -> Value {
        serde_json::from_str(self.body()).unwrap_or_else(|error| panic!("{error}: {}", self.text))
    }
}

impl Running {
    fn start(data: &Path, folder: &Path) -> Self {
        Self::start_with(data, folder, &[])
    }

    /// `start` with more flags.
    fn start_with(data: &Path, folder: &Path, more: &[&str]) -> Self {
        let mut child = guard::Guard::spawn(
            Command::new(env!("CARGO_BIN_EXE_ink"))
                .args(["serve", folder.to_str().unwrap(), "--addr", "127.0.0.1:0", "--data", data.to_str().unwrap()])
                // a file address is the only kind a test with no network can add
                // the boot barrier is the skew: a short one keeps the test quick
                .args(["--audience", "api.localhost", "--auth-skew", "1"])
                .args(["--git-scheme", "file", "--site-ttl", "0", "--poll-lookout", "1", "--worker-idle", "1"])
                .args(more),
        );
        let lines = Lines(child.lines());
        let address = lines.until("http://127.0.0.1:");
        let port = address.trim().strip_prefix("http://127.0.0.1:").unwrap().parse().unwrap();
        let key = keyed::Keyed::operator(data, "api.localhost");
        Self { lines, port, key, _child: child }
    }

    /// `method target` with `Host: host`, read to the end.
    fn fetch(&self, (method, target, host): (Verb, &str, &str), extra: &[(&str, String)], body: &str) -> Got {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        let mut headers = String::new();
        for (name, value) in extra {
            write!(headers, "{name}: {value}\r\n").unwrap();
        }
        write!(
            stream,
            "{} {target} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n{headers}Content-Length: {}\r\n\r\n{body}",
            method.name(),
            body.len()
        )
        .unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        Got { text: String::from_utf8_lossy(&bytes).into_owned() }
    }

    /// An API call as the operator, signed; asked again while the server is still quiet after its
    /// start.
    fn api(&self, route: (Verb, &str), body: &Value) -> Got {
        let body = body.to_string();
        loop {
            let auth = [
                ("Authorization", self.key.sign(&route.0.name(), route.1, body.as_bytes())),
                ("Content-Type", "application/json".to_owned()),
            ];
            let got = self.fetch((route.0, route.1, "api.localhost"), &auth, &body);
            if !got.text.contains("auth.server.restarting") {
                return got;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    /// A page of a site, by its host.
    fn page(&self, handle: &str, path: &str) -> Got {
        self.fetch((Verb::Get, path, &format!("{handle}.localhost")), &[], "")
    }
}

#[test]
fn a_git_site_is_added_served_updated_rolled_back_exported_and_deleted() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-selfhost");
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    let mut repo = Repo::new(&root.join("repo"));
    let first = repo.commit(&[], &[("index.md", "# Home\n\nThe first version.\n")]);
    let empty = root.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let ink = Running::start(&root.join("data"), &empty);

    let added = ink.api(
        (Verb::Post, "/v1/sites"),
        &json!({ "handle": "diary", "source": { "kind": "git", "url": format!("file://{}", repo.root.display()) } }),
    );
    assert_eq!(added.status(), "201", "{}", added.text);
    let site = added.json()["id"].as_str().unwrap().to_owned();
    let (builds, deploys, one) =
        (format!("/v1/sites/{site}/builds"), format!("/v1/sites/{site}/deploys"), "The first version.");

    // the watch finds the new site and queues its first build, the worker builds it
    ink.lines.until("watch: queued build");
    let built = loop {
        let line = ink.lines.until("worker: ");
        if !line.ends_with("worker: started") {
            break line;
        }
    };
    if built.contains("worker: failed") {
        // a binary built without its components cannot read a git source; it says so
        assert!(built.contains("not available"), "{built}");
        return;
    }
    assert!(built.contains("(live)"), "{built}");
    let page = ink.page("diary", "/");
    assert_eq!(page.status(), "200", "{}", page.text);
    assert!(page.body().contains(one), "{}", page.text);

    // a commit, then a build asked for by hand: the poll's own interval is far longer
    repo.commit(&[first], &[("index.md", "# Home\n\nThe second version.\n")]);
    assert_eq!(ink.api((Verb::Post, &builds), &json!({})).status(), "201");
    ink.lines.until("worker: built");
    let page = ink.page("diary", "/");
    assert!(page.body().contains("The second version."), "{}", page.text);

    // roll back to the first deploy
    let listed = ink.api((Verb::Get, &deploys), &json!({})).json();
    let items = listed["items"].as_array().unwrap();
    assert_eq!(items.len(), 2, "{listed}");
    let older = items.iter().find(|deploy| deploy["live"] == false).unwrap()["id"].clone();
    let rolled = ink.api((Verb::Post, &format!("/v1/sites/{site}/rollback")), &json!({ "deploy": older }));
    assert_eq!(rolled.status(), "200", "{}", rolled.text);
    assert!(ink.page("diary", "/").body().contains(one));

    // export, then delete
    let export = ink.api((Verb::Get, &format!("/v1/sites/{site}/export")), &json!({}));
    assert_eq!(export.status(), "200", "{}", export.text);
    assert!(export.body().len() > 512, "an archive of the site, not an error: {} bytes", export.body().len());
    let removed = ink.api((Verb::Delete, &format!("/v1/sites/{site}")), &json!({}));
    assert!(removed.status().starts_with('2'), "{}", removed.text);
    let gone = ink.page("diary", "/");
    assert_ne!(gone.status(), "200", "{}", gone.text);
    assert!(!gone.body().contains(one));
}

#[test]
fn behind_a_proxy_the_client_is_the_forwarded_address_and_passkey_challenges_are_limited_per_client() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-forwarded");
    let _ = std::fs::remove_dir_all(&root);
    let (data, folder) = (root.join("data"), root.join("folder"));
    std::fs::create_dir_all(&folder).unwrap();
    let running = Running::start_with(&data, &folder, &["--forwarded-hops", "1", "--challenge-limit", "2/1h"]);
    let challenge = |client: Option<&str>| {
        let mut headers = vec![("Sec-Fetch-Site", "same-origin".to_owned())];
        headers.extend(client.map(|client| ("X-Forwarded-For", client.to_owned())));
        loop {
            let got = running.fetch((Verb::Post, "/v1/passkeys/challenge", "api.localhost"), &headers, "");
            if !got.text.contains("auth.server.restarting") {
                return got;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    };
    // a proxy that sent nothing is the operator's mistake, said so, and not served
    let bare = challenge(None);
    assert_eq!(bare.status(), "400", "{}", bare.text);
    assert!(bare.body().contains("auth.forwarded.refused"), "{}", bare.text);
    // two tries a client, and a client is the address the proxy wrote, not the connection
    for _ in 0..2 {
        assert_eq!(challenge(Some("203.0.113.5")).status(), "200");
    }
    let limited = challenge(Some("203.0.113.5"));
    assert_eq!(limited.status(), "429", "{}", limited.text);
    assert!(limited.body().contains("auth.rate.limited"), "{}", limited.text);
    assert_eq!(challenge(Some("198.51.100.9")).status(), "200", "another client has its own count");
    // the recovery routes exist (migration 203 ran) and need a session
    let codes = running.fetch(
        (Verb::Get, "/v1/recovery/codes", "api.localhost"),
        &[("X-Forwarded-For", "203.0.113.5".to_owned())],
        "",
    );
    assert_eq!(codes.status(), "401", "{}", codes.text);
}
