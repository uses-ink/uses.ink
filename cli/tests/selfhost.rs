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
use std::io::{BufRead, BufReader, Read, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

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
    fn of(stream: impl Read + Send + 'static, into: &std::sync::mpsc::Sender<String>) {
        let into = into.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if into.send(line).is_err() {
                    return;
                }
            }
        });
    }

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
    child: Child,
    lines: Lines,
    port: u16,
    token: String,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
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
        let mut child = Command::new(env!("CARGO_BIN_EXE_ink"))
            .args(["serve", folder.to_str().unwrap(), "--addr", "127.0.0.1:0", "--data", data.to_str().unwrap()])
            // a file address is the only kind a test with no network can add
            .args(["--git-scheme", "file", "--site-ttl", "0", "--poll-lookout", "1", "--worker-idle", "1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let (into, received) = channel();
        Lines::of(child.stdout.take().unwrap(), &into);
        Lines::of(child.stderr.take().unwrap(), &into);
        let lines = Lines(received);
        let address = lines.until("http://127.0.0.1:");
        let port = address.trim().strip_prefix("http://127.0.0.1:").unwrap().parse().unwrap();
        let token = lines.until("owner token");
        let token = token.split_once("owner token").unwrap().1.split_whitespace().next().unwrap().to_owned();
        Self { child, lines, port, token }
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

    /// An API call as the owner.
    fn api(&self, route: (Verb, &str), body: &Value) -> Got {
        let auth =
            [("Authorization", format!("Bearer {}", self.token)), ("Content-Type", "application/json".to_owned())];
        self.fetch((route.0, route.1, "api.localhost"), &auth, &body.to_string())
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
