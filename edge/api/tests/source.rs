//! Source upload on real adapters: the tree, the blobs it is missing, the build it queues and the
//! files given back.

#![allow(clippy::unwrap_used, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

extern crate uses_ink_edge_api as edge_api;

mod support;

use build::{Leaf, Tree};
use edge_api::tar::{Limits as Reading, Tree as Files, Writer};
use edge_api::{Limits, Method, Status};
use model::{Bytes, ContentPath, Hash};
use serde_json::{Value, json};
use support::Rig;

fn leaf(path: &str, text: &str) -> Leaf {
    Leaf { path: ContentPath::parse(path).unwrap(), hash: Hash::of(text), size: Bytes(text.len() as u64), modified: None }
}

/// A batch of blobs named by their hashes.
fn batch(texts: &[&str]) -> Vec<u8> {
    let mut writer = Writer::new(1 << 20);
    for text in texts {
        writer.file(&Hash::of(text).to_string(), text.as_bytes()).unwrap();
    }
    writer.finish()
}

fn site(rig: &Rig) -> String {
    let body = json!({ "handle": "diary", "source": { "kind": "upload" } }).to_string();
    let (reply, value) = rig.call(Method::Post, "/v1/sites", body.as_bytes());
    assert_eq!(reply.status, Status::Created, "{value}");
    value["id"].as_str().unwrap().to_owned()
}

#[test]
fn a_tree_is_missing_blobs_until_they_are_sent_then_it_is_a_build() {
    let rig = Rig::new(Limits::default());
    let id = site(&rig);
    let target = format!("/v1/sites/{id}/source");
    let tree = Tree::of(vec![leaf("index.md", "# Hi\n"), leaf("notes/a.md", "a")]).unwrap();

    let (reply, value) = rig.call(Method::Post, &target, &tree.bytes());
    assert_eq!(reply.status, Status::Ok, "{value}");
    assert_eq!(value["state"], "missing");
    assert_eq!(value["missing"].as_array().unwrap().len(), 2);
    assert_eq!(value["part"], Limits::default().part);

    // an interrupted upload: one blob arrives, then the tree is sent again
    let (reply, value) = rig.call(Method::Put, &target, &batch(&["# Hi\n"]));
    assert_eq!(reply.status, Status::Ok, "{value}");
    assert_eq!((value["blobs"].as_u64(), value["created"].as_u64()), (Some(1), Some(1)));
    let (_, value) = rig.call(Method::Post, &target, &tree.bytes());
    assert_eq!(value["missing"], json!([Hash::of("a").to_string()]));

    rig.call(Method::Put, &target, &batch(&["a"]));
    let (reply, value) = rig.call(Method::Post, &target, &tree.bytes());
    assert_eq!(reply.status, Status::Created, "{value}");
    assert_eq!((value["state"].as_str(), value["files"].as_u64()), (Some("queued"), Some(2)));
    let build = value["build"].as_str().unwrap().to_owned();

    let (_, status) = rig.call(Method::Get, &format!("/v1/sites/{id}"), b"");
    assert_eq!(status["pending"][0]["id"], build);
    assert_eq!(status["pending"][0]["trigger"], "upload");

    let reply = rig.answer(&rig.request(&rig.operator, (Method::Get, &target), b""));
    assert_eq!(reply.status, Status::Ok);
    let files = Files::read(&reply.body, &Reading { files: 10, file: 100, total: 100, name: 100 }).unwrap();
    let held: Vec<_> = files.files().map(|(path, bytes)| (path.as_str().to_owned(), bytes.to_vec())).collect();
    assert_eq!(held, [("index.md".to_owned(), b"# Hi\n".to_vec()), ("notes/a.md".to_owned(), b"a".to_vec())]);
}

#[test]
fn a_blob_is_checked_against_its_name() {
    let rig = Rig::new(Limits::default());
    let target = format!("/v1/sites/{}/source", site(&rig));
    let mut writer = Writer::new(1 << 20);
    writer.file(&Hash::of("one").to_string(), b"two").unwrap();
    let (reply, value) = rig.call(Method::Put, &target, &writer.finish());
    assert_eq!((reply.status, value["problem"].as_str()), (Status::Unprocessable, Some("source.blob.mismatch")));
    let mut writer = Writer::new(1 << 20);
    writer.file("notes.md", b"x").unwrap();
    let (reply, value) = rig.call(Method::Put, &target, &writer.finish());
    assert_eq!((reply.status, value["problem"].as_str()), (Status::Unprocessable, Some("source.blob.unnamed")));
}

#[test]
fn a_tree_over_a_limit_or_for_a_git_site_is_refused_with_the_limit() {
    let limits = Limits { file: 4, ..Limits::default() };
    let rig = Rig::new(limits);
    let target = format!("/v1/sites/{}/source", site(&rig));
    let tree = Tree::of(vec![leaf("big.md", "more than four")]).unwrap();
    let (reply, value) = rig.call(Method::Post, &target, &tree.bytes());
    assert_eq!((reply.status, value["problem"].as_str()), (Status::TooLarge, Some("source.file.over")));
    let (reply, value) = rig.call(Method::Post, &target, b"{}");
    assert_eq!((reply.status, value["problem"].as_str()), (Status::BadRequest, Some("source.tree.unreadable")));

    let git = json!({ "handle": "repo", "source": { "kind": "git", "url": "https://example.org/a.git" } });
    let (_, made) = rig.call(Method::Post, "/v1/sites", git.to_string().as_bytes());
    let target = format!("/v1/sites/{}/source", made["id"].as_str().unwrap());
    let (reply, value) = rig.call(Method::Post, &target, &Tree::of(vec![]).unwrap().bytes());
    assert_eq!((reply.status, value["problem"].as_str()), (Status::Unprocessable, Some("source.site.not-upload")));
}

#[test]
fn a_site_with_no_push_has_no_source_to_give() {
    let rig = Rig::new(Limits::default());
    let (reply, value) = rig.call(Method::Get, &format!("/v1/sites/{}/source", site(&rig)), b"");
    assert_eq!((reply.status, value["problem"].as_str()), (Status::NotFound, Some("source.tree.none")));
    let _: Value = value;
}
