//! Previews of branches through the real binary (decision 0091): a git repository on disk with a
//! second branch is added as a site, the watch finds the branch, the worker builds it as a
//! preview served at its own address, `ink preview list` shows it and `ink preview remove` ends
//! it. The test waits on the server's own log lines, never on a long sleep.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines,
    reason = "tests stop at the first surprise"
)]

mod keyed;
mod rig;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gix::ObjectId;
use gix::objs::tree::EntryKind;
use rig::{Machine, Server};
use serde_json::json;

/// How long a build of a small site may take on a loaded machine.
const PATIENCE: Duration = Duration::from_secs(90);

/// A repository written object by object, one commit per call, on the branch named.
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

    fn commit(&mut self, branch: &str, parents: &[ObjectId], text: &str) -> ObjectId {
        let mut editor = self.objects.edit_tree(ObjectId::empty_tree(gix::hash::Kind::Sha1)).unwrap();
        editor.upsert("index.md", EntryKind::Blob, self.objects.write_blob(text).unwrap().detach()).unwrap();
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
        std::fs::write(self.root.join(".git/refs/heads").join(branch), format!("{id}\n")).unwrap();
        std::fs::write(self.root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        id
    }
}

/// Waits for `ready` to hold, asking again every quarter second.
fn until(what: &str, server: &Server, ready: impl Fn(&str) -> bool) {
    let started = Instant::now();
    while !ready(&server.said()) {
        assert!(started.elapsed() < PATIENCE, "{what} did not happen: {}", server.said());
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// A server watching a repository that has `main` and `draft`, with the site `diary` added.
fn start(name: &str, flags: &[&str]) -> (Server, Repo, String) {
    let mut every = vec!["--git-scheme", "file", "--site-ttl", "0", "--poll-lookout", "1", "--worker-idle", "1"];
    every.extend_from_slice(flags);
    let server = Server::start(name, &every);
    let mut repo = Repo::new(&server.root.join("repo"));
    let main = repo.commit("main", &[], "# Home\n\nThe main branch.\n");
    repo.commit("draft", &[main], "# Home\n\nThe draft branch.\n");
    let key = keyed::Keyed::operator(&server.data, &server.audience);
    let body =
        json!({ "handle": "diary", "source": { "kind": "git", "url": format!("file://{}", repo.root.display()) } })
            .to_string();
    // each try is signed anew: the server answers `restarting` through its boot barrier, which is
    // longer than a signature is good for
    let added = loop {
        let auth = key.sign("POST", "/v1/sites", body.as_bytes());
        let headers = [("Authorization", auth.as_str()), ("Content-Type", "application/json")];
        let got = server.once(("POST", "/v1/sites"), &headers, &body);
        if !got.text.contains("auth.server.restarting") {
            break got;
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    assert_eq!(added.status(), "201", "{}", added.text);
    let site = added.json()["id"].as_str().unwrap().to_owned();
    (server, repo, site)
}

/// A machine signed in as the operator, with a folder to run in.
fn operator(server: &Server) -> Machine<'_> {
    let mut machine = server.machine("operator");
    machine.dir = server.root.join("site");
    std::fs::create_dir_all(&machine.dir).unwrap();
    let data = server.data.to_str().unwrap().to_owned();
    let logged = machine.ink(&["login", "--data", &data, "--label", "operator"]);
    assert_eq!(logged.code, 0, "{}", logged.said());
    machine
}

#[test]
fn a_branch_of_a_git_site_is_built_as_a_preview_listed_served_and_removed() {
    let (server, _repo, site) = start("previews-branch", &[]);
    until("the preview", &server, |log| log.contains("preview=draft"));
    let machine = operator(&server);
    let listed = machine.ink(&["preview", "list", "--site", &site, "--json"]);
    assert_eq!(listed.code, 0, "{}", listed.said());
    let previews = listed.json()["previews"].as_array().unwrap().clone();
    let slugs: Vec<&str> = previews.iter().map(|p| p["slug"].as_str().unwrap()).collect();
    assert_eq!(slugs, ["draft"], "the site's own branch is not a preview: {}", listed.out);
    assert!(previews[0]["url"].as_str().unwrap().contains("draft--diary."), "{}", listed.out);
    let plain = machine.ink(&["preview", "list", "--site", &site]);
    assert!(plain.out.contains("draft"), "{}", plain.said());

    let page = server.page("draft--diary", "/");
    assert_eq!(page.status(), "200", "{}", page.text);
    assert!(page.text.contains("The draft branch."), "{}", page.text);
    until("the site's own build", &server, |log| log.contains("state=live"));
    assert!(server.page("diary", "/").text.contains("The main branch."));

    let removed = machine.ink(&["preview", "remove", "draft", "--site", &site, "--json"]);
    assert_eq!(removed.code, 0, "{}", removed.said());
    assert_eq!(removed.json()["slug"], "draft");
    assert_ne!(server.page("draft--diary", "/").status(), "200");
    let again = machine.ink(&["preview", "remove", "draft", "--site", &site]);
    assert_eq!(again.code, 2, "{}", again.said());
    assert!(again.err.contains("preview.label.unknown"), "{}", again.err);
    let empty = machine.ink(&["preview", "list", "--site", &site, "--json"]);
    assert_eq!(empty.json()["previews"].as_array().unwrap().len(), 0, "{}", empty.out);
    let bad = machine.ink(&["preview", "remove", "Not A Label", "--site", &site]);
    assert_eq!(bad.code, 2, "{}", bad.said());
}

#[test]
fn zero_branches_turns_the_previews_off() {
    let (server, _repo, _site) = start("previews-off", &["--preview-branches", "0"]);
    until("the site's own build", &server, |log| log.contains("state=live"));
    assert!(!server.said().contains("queued preview build"), "{}", server.said());
    assert_eq!(server.page("draft--diary", "/").status(), "404");
}
