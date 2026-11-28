//! The publish verbs through the real binary: `ink serve --data`, `ink login --data`, then
//! `ink init`, `push`, `status`, `watch` and `export` against it, and the ways each one stops.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::too_many_lines,
    clippy::indexing_slicing,
    reason = "tests stop at the first surprise"
)]

mod guard;
mod keyed;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use edge_api::tar::Writer;
use model::Hash;

/// What a run of `ink` said.
struct Ran {
    code: i32,
    out: String,
    err: String,
}

impl Ran {
    /// stdout, trimmed: the bare id a script reads.
    fn answer(&self) -> &str {
        self.out.trim()
    }

    /// The JSON document of a `--json` run.
    fn json(&self) -> serde_json::Value {
        serde_json::from_str(self.out.lines().last().unwrap()).unwrap_or_else(|error| panic!("{error}: {}", self.out))
    }
}

/// A running `ink serve` with a data folder, a signed-in home and a folder to publish.
struct Rig {
    child: Option<guard::Guard>,
    root: PathBuf,
    data: PathBuf,
    home: PathBuf,
    site: PathBuf,
    port: u16,
    api: String,
    audience: String,
    log: PathBuf,
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Rig {
    /// Starts the server and signs in.
    fn start(name: &str, extra: &[&str]) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("cli-push-{name}"));
        std::fs::remove_dir_all(&root).ok();
        let (data, home, site) = (root.join("data"), root.join("home"), root.join("site"));
        let served = root.join("served");
        for dir in [&site, &served] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::write(served.join("index.html"), "served").unwrap();
        let log = root.join("serve.log");
        // the audience names the port, so the port is chosen first; a port that another process took
        // in between ends the server at once, and another one is tried
        let (child, port) = (0..5)
            .find_map(|_| {
                let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
                let (addr, audience) = (format!("127.0.0.1:{port}"), format!("api.localhost:{port}"));
                let sink = std::fs::File::create(&log).unwrap();
                let mut child = guard::Guard::adopt(
                    Command::new(env!("CARGO_BIN_EXE_ink"))
                        .args(["serve", served.to_str().unwrap(), "--addr", &addr, "--data", data.to_str().unwrap()])
                        .args(["--audience", &audience, "--auth-skew", "5"])
                        .args(extra)
                        .stdin(Stdio::null())
                        .stdout(sink.try_clone().unwrap())
                        .stderr(sink)
                        .spawn()
                        .unwrap(),
                );
                let started = std::time::Instant::now();
                while !std::fs::read_to_string(&log).unwrap_or_default().contains("backing off") {
                    assert!(started.elapsed() < Duration::from_secs(60), "the server did not start");
                    if child.ended() {
                        return None;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                Some((child, port))
            })
            .unwrap_or_else(|| panic!("the server ended five times before it started"));
        let (audience, api) = (format!("api.localhost:{port}"), format!("http://api.localhost:{port}"));
        let rig = Self { child: Some(child), root, data, home, site, port, api, audience, log };
        let signed =
            rig.ink_in(&rig.home, &rig.root, &["login", "--data", rig.data.to_str().unwrap(), "--label", "e2e"]);
        assert_eq!(signed.code, 0, "{}{}", signed.out, signed.err);
        rig
    }

    /// What the server has said so far.
    fn said(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    /// Stops the server.
    fn stop(&mut self) {
        // the guard kills and reaps the child when it is dropped
        self.child.take();
    }

    /// `ink <args>` in `dir`, as the user of `home`, retried while the server has just restarted.
    fn ink_in(&self, home: &Path, dir: &Path, args: &[&str]) -> Ran {
        let mut ran = self.once(home, dir, args);
        for _ in 0..80 {
            if !ran.err.contains("auth.server.restarting") && !ran.err.contains("deploy.api.slow") {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
            ran = self.once(home, dir, args);
        }
        ran
    }

    fn once(&self, home: &Path, dir: &Path, args: &[&str]) -> Ran {
        let out = Command::new(env!("CARGO_BIN_EXE_ink"))
            .arg("-C")
            .arg(dir)
            .args(args)
            .env_clear()
            .env("HOME", home)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("INK_API", &self.api)
            .env("INK_TIMEOUT", "20")
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .output()
            .unwrap();
        Ran {
            code: out.status.code().unwrap_or(-1),
            out: String::from_utf8_lossy(&out.stdout).into_owned(),
            err: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }

    /// `ink <args>` in the site's folder, as the signed-in user.
    fn ink(&self, args: &[&str]) -> Ran {
        self.ink_in(&self.home, &self.site, args)
    }

    /// Writes `text` at `name` in the site's folder.
    fn write(&self, name: &str, text: &str) {
        let at = self.site.join(name);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }

    /// One raw signed request, answered in full.
    fn raw(&self, (method, target): (&str, &str), body: &[u8]) -> String {
        let helper = keyed::Keyed::operator(&self.data, &self.audience);
        let authorization = helper.sign(method, target, body);
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let head = format!(
            "{method} {target} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nAuthorization: {authorization}\r\nContent-Length: {}\r\n\r\n",
            self.audience,
            body.len()
        );
        stream.write_all(head.as_bytes()).unwrap();
        stream.write_all(body).unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// Links the site's folder to a new site with `handle`; returns the site's id.
    fn init(&self, handle: &str) -> String {
        let init = self.ink(&["init", "--handle", handle, "--json"]);
        assert_eq!(init.code, 0, "{}{}{}", init.out, init.err, self.said());
        init.json()["site"].as_str().unwrap().to_owned()
    }
}

#[test]
fn a_folder_is_linked_pushed_inspected_watched_and_exported() {
    let rig = Rig::start("flow", &[]);
    rig.write("index.md", "# One\n");
    rig.write("notes/a.txt", "first note");

    let init = rig.ink(&["init", "--handle", "diary", "--json"]);
    assert_eq!(init.code, 0, "{}{}", init.out, init.err);
    let linked = init.json();
    assert_eq!(linked["schema"], "uses.link/v1");
    assert_eq!(linked["created"], true);
    let site = linked["site"].as_str().unwrap().to_owned();
    let file = std::fs::read_to_string(rig.site.join(".uses/site")).unwrap();
    assert!(file.contains(&site) && file.contains("uses.link/v1"), "{file}");
    assert!(!file.contains("ed25519") && !file.contains("token"), "the link holds a credential: {file}");

    // the same init again changes nothing; another site is refused without --force
    let again = rig.ink(&["init", "--site", &site, "--json"]);
    assert_eq!(again.code, 0, "{}{}", again.out, again.err);
    assert_eq!(again.json()["unchanged"], true);
    let refused = rig.ink(&["init", "--handle", "other"]);
    assert_eq!(refused.code, 2, "{}{}", refused.out, refused.err);
    assert!(refused.err.contains("link.file.exists") && refused.err.contains("--force"), "{}", refused.err);

    let pushed = rig.ink(&["push", "--wait", "30", "--json"]);
    assert_eq!(pushed.code, 0, "{}{}{}", pushed.out, pushed.err, rig.said());
    let first = pushed.json();
    assert_eq!(first["schema"], "uses.push/v1");
    assert_eq!(first["state"], "succeeded");
    assert_eq!(first["live"], true);
    assert_eq!(first["files"], 2, "the link file is not content: {first}");
    assert_eq!(first["sent"], 2);
    let deploy = first["deploy"].as_str().unwrap().to_owned();
    let build = first["build"].as_str().unwrap().to_owned();

    // the bare id is what stdout carries in text mode
    rig.write("notes/a.txt", "second note");
    rig.write("index.md", "# Two\n");
    let second = rig.ink(&["push", "--wait", "30"]);
    assert_eq!(second.code, 0, "{}{}", second.out, second.err);
    assert_eq!(second.answer().len(), 64, "{}", second.out);
    assert_ne!(second.answer(), deploy);
    assert!(second.err.contains("Sent") && second.err.contains("Built"), "{}", second.err);

    let status = rig.ink(&["status"]);
    assert_eq!(status.code, 0, "{}{}", status.out, status.err);
    assert!(
        status.out.contains("diary") && status.out.contains("ok") && status.out.contains("domains"),
        "{}",
        status.out
    );
    let json = rig.ink(&["status", "--json"]).json();
    assert_eq!(json["schema"], "uses.status/v1");
    assert_eq!(json["health"], "ok");
    assert_eq!(json["live"]["deploy"], second.answer());

    // a finished build is followed to the same result
    let watched = rig.ink(&["watch", &build[..8], "--json"]);
    assert_eq!(watched.code, 0, "{}{}", watched.out, watched.err);
    assert_eq!(watched.json()["schema"], "uses.watch/v1");
    assert_eq!(watched.json()["deploy"], deploy);

    // the site's files, and the source of the last push, each into a new folder
    let out = rig.ink(&["export", "built", "--json"]);
    assert_eq!(out.code, 0, "{}{}", out.out, out.err);
    assert!(rig.site.join("built").is_dir());
    let source = rig.ink(&["export", "--source", "src", "--json"]);
    assert_eq!(source.code, 0, "{}{}", source.out, source.err);
    assert_eq!(source.json()["files"], 2);
    assert_eq!(std::fs::read_to_string(rig.site.join("src/notes/a.txt")).unwrap(), "second note");
    assert!(!rig.site.join("src/.uses").exists());
    let older = rig.ink(&["export", "--deploy", &deploy[..12], "older"]);
    assert_eq!(older.code, 0, "{}{}", older.out, older.err);

    // a folder that holds something is not written into, and nothing partial is left
    let busy = rig.ink(&["export", "src"]);
    assert_eq!(busy.code, 2, "{}{}", busy.out, busy.err);
    assert!(busy.err.contains("export.folder.occupied"), "{}", busy.err);
    let partial: Vec<_> = std::fs::read_dir(&rig.site)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains("partial"))
        .collect();
    assert!(partial.is_empty(), "{partial:?}");

    // the export limits are settings: a file larger than the limit is refused, and leaves nothing
    let small = rig.ink(&["export", "--source", "tiny", "--max-file", "3"]);
    assert_ne!(small.code, 0, "{}{}", small.out, small.err);
    assert!(small.err.contains("export.archive.invalid"), "{}", small.err);
    assert!(!rig.site.join("tiny").exists());
}

#[test]
fn a_folder_with_no_link_is_refused_and_so_is_a_bad_signature() {
    let rig = Rig::start("refusals", &[]);
    rig.write("index.html", "x");
    for verb in [&["push"][..], &["status"], &["watch"], &["export"]] {
        let ran = rig.ink(verb);
        assert_eq!(ran.code, 2, "{verb:?}: {}{}", ran.out, ran.err);
        assert!(ran.err.contains("link.file.missing") && ran.err.contains("ink init"), "{verb:?}: {}", ran.err);
    }
    // the config file's site never stands in for a link
    let site = rig.init("garden");
    std::fs::remove_dir_all(rig.site.join(".uses")).unwrap();
    let ran = rig.ink(&["status"]);
    assert_eq!(ran.code, 2, "{}{}", ran.out, ran.err);

    // a machine that holds no key is refused with 3
    std::fs::create_dir_all(rig.site.join(".uses")).unwrap();
    std::fs::write(
        rig.site.join(".uses/site"),
        format!("schema = \"uses.link/v1\"\napi = \"{}\"\nsite = \"{site}\"\n", rig.api),
    )
    .unwrap();
    let stranger = rig.root.join("stranger");
    std::fs::create_dir_all(&stranger).unwrap();
    let ran = rig.ink_in(&stranger, &rig.site, &["push", "--wait", "30"]);
    assert_eq!(ran.code, 3, "{}{}", ran.out, ran.err);
}

#[test]
fn a_server_that_is_gone_is_exit_4_and_a_push_after_a_rollback_is_exit_5() {
    let mut rig = Rig::start("conflict", &[]);
    rig.write("index.md", "# One\n");
    let site = rig.init("diary");
    let first = rig.ink(&["push", "--wait", "30"]);
    assert_eq!(first.code, 0, "{}{}", first.out, first.err);
    rig.write("index.md", "# Two\n");
    assert_eq!(rig.ink(&["push", "--wait", "30"]).code, 0);
    let rolled = rig.ink(&["deploy", "rollback", &first.answer()[..12], "--site", &site]);
    assert_eq!(rolled.code, 0, "{}{}", rolled.out, rolled.err);

    rig.write("index.md", "# Three\n");
    let held = rig.ink(&["push", "--wait", "30", "--json"]);
    assert_eq!(held.code, 5, "{}{}", held.out, held.err);
    assert!(
        held.err.contains("ink deploy resume") || held.out.contains("ink deploy resume"),
        "{}{}",
        held.out,
        held.err
    );
    let status = rig.ink(&["status", "--json"]).json();
    assert_eq!(status["health"], "paused");
    assert_eq!(rig.ink(&["deploy", "resume", "--site", &site]).code, 0);

    rig.stop();
    let down = rig.ink(&["status", "--retries", "1", "--retry-wait", "10", "--timeout", "2"]);
    assert_eq!(down.code, 4, "{}{}", down.out, down.err);
    let down = rig.ink(&["push", "--wait", "30", "--retries", "0"]);
    assert_eq!(down.code, 4, "{}{}", down.out, down.err);
}

#[test]
fn an_interrupted_upload_is_resumed_by_pushing_again() {
    let rig = Rig::start("resume", &["--upload-part", "5000"]);
    let (one, two, three) = ("a".repeat(1500), "b".repeat(1500), "c".repeat(1500));
    rig.write("index.html", &one);
    rig.write("two.txt", &two);
    rig.write("three.txt", &three);
    let site = rig.init("diary");

    // part of the files reach the site, as when a push was cut off after its first request
    let mut writer = Writer::new(u64::MAX);
    writer.file(&Hash::of(one.as_bytes()).to_string(), one.as_bytes()).unwrap();
    let stored = rig.raw(("PUT", &format!("/v1/sites/{site}/source")), &writer.finish());
    assert!(stored.starts_with("HTTP/1.1 200"), "{stored}");

    let pushed = rig.ink(&["push", "--wait", "30", "--json", "--batch", "1600"]);
    assert_eq!(pushed.code, 0, "{}{}", pushed.out, pushed.err);
    let document = pushed.json();
    assert_eq!(document["files"], 3);
    assert_eq!(document["held"], 1, "{document}");
    assert_eq!(document["sent"], 2, "{document}");

    // a push of what the site holds already sends nothing
    let again = rig.ink(&["push", "--wait", "30", "--json"]);
    assert_eq!(again.code, 0, "{}{}", again.out, again.err);
    assert_eq!(again.json()["sent"], 0);
    assert_eq!(again.json()["held"], 3);
}

#[test]
fn a_tree_over_the_servers_limit_is_refused_and_says_so() {
    let rig = Rig::start("limits", &["--upload-tree", "300"]);
    for index in 0..8 {
        rig.write(&format!("page-{index}.txt"), "text");
    }
    rig.init("diary");
    let pushed = rig.ink(&["push", "--wait", "30"]);
    assert_ne!(pushed.code, 0, "{}{}", pushed.out, pushed.err);
    assert!(pushed.err.contains("deploy.api.refused"), "{}", pushed.err);
}
