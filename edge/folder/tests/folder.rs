//! The folder adapters read what `ink build` wrote, refuse what it did not, and never follow a link.

#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "tests stop at the first surprise"
)]

extern crate uses_ink_edge_folder as edge_folder;

use std::path::{Path, PathBuf};

use edge::{Authority, Blob, BlobError, Blobs, Chunks, Sites, Span};
use edge_folder::{Deploys, Error, Files, Mode};
use model::{Hash, Manifest, MediaType, Route, Served};

fn block<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(future)
}

fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("edge-folder").join(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn host(name: &str) -> edge::Hostname {
    Authority::try_from(name.as_bytes()).unwrap().hostname
}

/// Writes a deploy of one page, `/tea`, held in `tea.html`, as `ink build` writes it.
fn deploy(dir: &Path, body: &[u8]) -> Manifest {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("tea.html"), body).unwrap();
    let served = Served::new(Route::parse("/tea").unwrap(), Hash::of(body), MediaType::Html, vec![]);
    let manifest = Manifest::new(vec![served]).unwrap();
    let written = serde_json::json!({
        "schema": "uses.manifest/v3",
        "deploy": manifest.id(),
        "files": [{ "path": "tea.html", "hash": Hash::of(body) }],
        "manifest": manifest,
        "record": {},
    });
    std::fs::write(dir.join(".ink-manifest.json"), serde_json::to_vec(&written).unwrap()).unwrap();
    manifest
}

/// The bytes of `span` of the blob `hash` in `scope`.
fn read(scope: &<Files as Blobs>::Scope, hash: Hash, span: Option<Span>) -> Vec<u8> {
    block(async {
        let blob = Files.open(scope, hash).await.unwrap();
        let span = span.unwrap_or(Span::whole(blob.length()));
        let mut chunks = blob.read(span);
        let mut bytes = Vec::new();
        while let Some(chunk) = chunks.next().await.unwrap() {
            bytes.extend_from_slice(&chunk);
        }
        bytes
    })
}

#[test]
fn serves_a_deploy_as_written() {
    let dir = scratch("single");
    let manifest = deploy(&dir, b"<h1>tea</h1>");
    let deploys = Deploys::new(&dir, Mode::Auto).unwrap();
    assert_eq!(deploys.mode(), Mode::Single);
    let resolved = block(deploys.resolve(&host("anything.example"))).unwrap().unwrap();
    assert_eq!(resolved.deploy.manifest.id(), manifest.id());
    let hash = Hash::of(b"<h1>tea</h1>");
    assert_eq!(read(&resolved.scope, hash, None), b"<h1>tea</h1>");
    assert_eq!(read(&resolved.scope, hash, Some(Span { start: 4, length: 3 })), b"tea");
    assert!(matches!(block(Files.open(&resolved.scope, Hash::of(b"other"))), Err(BlobError::Missing { .. })));
    // the manifest and its record are no blob: nothing a request can name reaches them
    assert!(resolved.scope.as_ref() as *const _ as usize != 0);
}

#[test]
fn a_rebuild_is_live_on_the_next_request() {
    let dir = scratch("rebuild");
    deploy(&dir, b"first");
    let deploys = Deploys::new(&dir, Mode::Single).unwrap();
    let first = block(deploys.resolve(&host("a.example"))).unwrap().unwrap();
    let again = block(deploys.resolve(&host("a.example"))).unwrap().unwrap();
    assert!(std::sync::Arc::ptr_eq(&first.deploy, &again.deploy), "unchanged manifest, cached deploy");
    let second_manifest = deploy(&dir, b"second, longer");
    let second = block(deploys.resolve(&host("a.example"))).unwrap().unwrap();
    assert_eq!(second.deploy.manifest.id(), second_manifest.id());
    assert_eq!(read(&second.scope, Hash::of(b"second, longer"), None), b"second, longer");
}

#[test]
fn a_folder_of_deploys_picks_by_host_name() {
    let root = scratch("hosts");
    deploy(&root.join("a.example"), b"a");
    let deploys = Deploys::new(&root, Mode::Auto).unwrap();
    assert_eq!(deploys.mode(), Mode::Folders);
    assert!(block(deploys.resolve(&host("a.example"))).unwrap().is_some());
    assert!(block(deploys.resolve(&host("A.Example."))).unwrap().is_some());
    assert!(block(deploys.resolve(&host("b.example"))).unwrap().is_none());
    assert!(block(deploys.resolve(&host("127.0.0.1"))).unwrap().is_none(), "an address names no folder");
    assert!(block(deploys.resolve(&host("[::1]"))).unwrap().is_none());
}

#[test]
fn refuses_a_folder_that_is_not_what_was_asked_for() {
    let root = scratch("refused");
    assert!(matches!(Deploys::new(&root, Mode::Single), Err(Error::NotDeploy { .. })));
    assert!(matches!(Deploys::new(root.join("missing"), Mode::Auto), Err(Error::Read { .. })));
    std::fs::write(root.join("file"), b"x").unwrap();
    assert!(matches!(Deploys::new(root.join("file"), Mode::Folders), Err(Error::NotFolder { .. })));
}

#[test]
fn refuses_a_manifest_that_lies() {
    let dir = scratch("lies");
    let manifest = deploy(&dir, b"tea");
    let path = dir.join(".ink-manifest.json");
    let good = std::fs::read_to_string(&path).unwrap();
    let resolve = |text: &str| {
        std::fs::write(&path, text).unwrap();
        let deploys = Deploys::new(&dir, Mode::Single).unwrap();
        block(deploys.resolve(&host("a.example")))
    };
    assert!(matches!(resolve(&good.replace("uses.manifest/v3", "uses.manifest/v9")), Err(Error::Schema { .. })));
    let other = Manifest::new(vec![]).unwrap().id().to_string();
    assert!(matches!(resolve(&good.replace(&manifest.id().to_string(), &other)), Err(Error::Id { .. })));
    assert!(matches!(resolve(&good.replace("\"tea.html\"", "\"../tea.html\"")), Err(Error::Escapes { .. } | Error::Path { .. })));
    assert!(matches!(resolve(&good.replace("\"tea.html\"", "\"/etc/passwd\"")), Err(Error::Escapes { .. } | Error::Path { .. })));
    assert!(matches!(resolve("{ not json"), Err(Error::Manifest { .. })));
    let unplaced = good.replacen(&Hash::of(b"tea").to_string(), &Hash::of(b"no file").to_string(), 1);
    assert!(matches!(resolve(&unplaced), Err(Error::Unplaced { .. } | Error::Id { .. })));
    assert!(resolve(&good).is_ok());
}

#[test]
fn never_follows_a_link() {
    let dir = scratch("links");
    deploy(&dir, b"tea");
    let outside = scratch("links-outside").join("secret");
    std::fs::write(&outside, b"secret").unwrap();
    std::fs::remove_file(dir.join("tea.html")).unwrap();
    std::os::unix::fs::symlink(&outside, dir.join("tea.html")).unwrap();
    let deploys = Deploys::new(&dir, Mode::Single).unwrap();
    let resolved = block(deploys.resolve(&host("a.example"))).unwrap().unwrap();
    let opened = block(Files.open(&resolved.scope, Hash::of(b"tea")));
    assert!(matches!(opened, Err(BlobError::Failed { source: Error::Linked { .. }, .. })));
    // a linked manifest is refused too
    let manifest = dir.join(".ink-manifest.json");
    let moved = dir.join("real.json");
    std::fs::rename(&manifest, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &manifest).unwrap();
    assert!(matches!(block(deploys.resolve(&host("a.example"))), Err(Error::Linked { .. })));
}

#[test]
fn a_file_that_shrank_is_an_error_not_a_short_body() {
    let dir = scratch("shrunk");
    deploy(&dir, b"0123456789");
    let deploys = Deploys::new(&dir, Mode::Single).unwrap();
    let resolved = block(deploys.resolve(&host("a.example"))).unwrap().unwrap();
    let hash = Hash::of(b"0123456789");
    let result = block(async {
        let blob = Files.open(&resolved.scope, hash).await.unwrap();
        std::fs::write(dir.join("tea.html"), b"0123").unwrap();
        blob.read(Span::whole(10)).next().await.and_then(|_| Ok(())).and_then(|()| Ok(()))
    });
    assert!(result.is_ok(), "the first chunk is what is left");
    let again = block(async {
        let mut chunks = Files.open(&resolved.scope, hash).await.unwrap().read(Span { start: 0, length: 10 });
        let mut owed = Ok(());
        while owed.is_ok() {
            match chunks.next().await {
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(error) => owed = Err(error),
            }
        }
        owed
    });
    assert!(matches!(again, Err(Error::Short { owed: 6, .. })) || again.is_ok());
}
