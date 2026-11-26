//! Folders read through the public API, in throwaway folders under cargo's target directory.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

extern crate uses_ink_host_native as host_native;

use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use engine::Blobs;
use host_native::{Budget, Directory, Folder, Memory, Refused, Store};
use model::{Bytes, ContentPath, Diagnostic, Known, Provenance, Version};

/// A fresh folder named for the test, holding `files`.
fn site(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("host-native").join(name);
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn path(text: &str) -> ContentPath {
    text.parse().unwrap()
}

fn code(refused: &Refused) -> &'static str {
    Diagnostic::from(refused).code()
}

#[test]
fn reads_every_file_in_order_but_version_control() {
    let root = site(
        "order",
        &[("b.md", "b"), ("a/z.md", "z"), ("a-b.md", "ab"), (".git/HEAD", "x"), (".jj/repo", "x"), (".hidden", "h")],
    );
    let mut store = Memory::default();
    let read = Folder::new(&root).read(&mut store).unwrap();
    let paths: Vec<&str> = read.snapshot.tree().iter().map(|entry| entry.path.as_str()).collect();
    assert_eq!(paths, [".hidden", "a-b.md", "a/z.md", "b.md"]);
    let entry = read.snapshot.entry(&path("a/z.md")).unwrap();
    assert_eq!(entry.size, Bytes(1));
    assert_eq!(store.read(&entry.blob).unwrap().as_ref(), b"z");
    let facts = read.snapshot.facts_of(&path("b.md")).unwrap();
    assert!(matches!(facts.updated, Known::NotAfter { from: Provenance::Filesystem, .. }));
    assert!(matches!(facts.created, Known::NotAfter { from: Provenance::Filesystem, .. }));
    assert!(facts.updated.exact().is_none());
    let again = Folder::new(&root).read(&mut Memory::default()).unwrap();
    assert_eq!(again.snapshot.version(), read.snapshot.version());
    assert!(matches!(read.snapshot.version(), Version::Folder { .. }));
}

#[test]
fn the_version_follows_the_bytes() {
    let root = site("version", &[("a.md", "one")]);
    let before = Folder::new(&root).read(&mut Memory::default()).unwrap();
    std::fs::write(root.join("a.md"), "two").unwrap();
    let after = Folder::new(&root).read(&mut Memory::default()).unwrap();
    assert_ne!(before.snapshot.version(), after.snapshot.version());
}

#[test]
fn skips_the_output_and_says_so() {
    let root = site("output", &[("a.md", "a"), ("out/index.html", "<p>")]);
    let folder = Folder { output: vec![path("out")], ..Folder::new(&root) };
    let read = folder.read(&mut Memory::default()).unwrap();
    assert_eq!(read.snapshot.tree().len(), 1);
    assert_eq!(read.skipped.len(), 1);
    assert_eq!(read.skipped[0].path, path("out"));
}

#[test]
fn refuses_a_link_out_of_the_folder() {
    let outside = site("outside-target", &[("secret.md", "s")]);
    let root = site("outside", &[("a.md", "a")]);
    symlink(outside.join("secret.md"), root.join("leak.md")).unwrap();
    let refused = Folder::new(&root).read(&mut Memory::default()).unwrap_err();
    assert!(matches!(&refused, Refused::Outside { path, .. } if path.as_str() == "leak.md"));
    assert_eq!(code(&refused), "source.link.outside");
}

#[test]
fn follows_links_inside_and_refuses_a_cycle() {
    let root = site("inside", &[("notes/a.md", "a")]);
    symlink(root.join("notes/a.md"), root.join("b.md")).unwrap();
    let read = Folder::new(&root).read(&mut Memory::default()).unwrap();
    assert!(read.snapshot.entry(&path("b.md")).is_some());
    symlink(root.join("notes"), root.join("notes/again")).unwrap();
    let refused = Folder::new(&root).read(&mut Memory::default()).unwrap_err();
    assert!(matches!(&refused, Refused::Cycle { path, .. } if path.as_str() == "notes/again"));
    assert_eq!(code(&refused), "source.link.cycle");
}

#[test]
fn refuses_a_name_no_site_holds() {
    let root = site("name", &[("a\\b.md", "a")]);
    let refused = Folder::new(&root).read(&mut Memory::default()).unwrap_err();
    assert!(matches!(&refused, Refused::Name { written, .. } if written == "a\\b.md"));
    assert_eq!(code(&refused), "source.name.invalid");
}

#[test]
fn refuses_past_each_budget_with_the_numbers() {
    let root = site("budget", &[("a.md", "aaaa"), ("b.md", "bbbb"), ("c.md", "cccc")]);
    let budget = Budget { files: 2, ..Budget::default() };
    let refused = Folder { budget, ..Folder::new(&root) }.read(&mut Memory::default()).unwrap_err();
    assert!(matches!(refused, Refused::Files { count: 3, limit: 2 }));
    assert_eq!(code(&refused), "source.budget.files");

    let budget = Budget { file: Bytes(3), ..Budget::default() };
    let refused = Folder { budget, ..Folder::new(&root) }.read(&mut Memory::default()).unwrap_err();
    assert!(matches!(&refused, Refused::File { path, size: Bytes(4), limit: Bytes(3) } if path.as_str() == "a.md"));
    assert_eq!(code(&refused), "source.budget.file");

    let budget = Budget { total: Bytes(10), ..Budget::default() };
    let refused = Folder { budget, ..Folder::new(&root) }.read(&mut Memory::default()).unwrap_err();
    assert!(matches!(refused, Refused::Total { size: Bytes(12), limit: Bytes(10) }));
    assert_eq!(code(&refused), "source.budget.bytes");
}

#[test]
fn refuses_a_missing_folder() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("host-native/never-made");
    let refused = Folder::new(&root).read(&mut Memory::default()).unwrap_err();
    assert!(matches!(refused, Refused::Root { .. }));
    assert_eq!(code(&refused), "source.root.unreadable");
}

#[test]
fn a_directory_store_keeps_blobs_by_hash() {
    let root = site("store", &[]);
    let mut store = Directory::open(root.join("blobs")).unwrap();
    let hash = store.put(b"ink".to_vec()).unwrap();
    assert_eq!(store.put(b"ink".to_vec()).unwrap(), hash);
    assert_eq!(store.read(&hash).unwrap().as_ref(), b"ink");
    assert!(store.path(&hash).ends_with(hash.to_string()));
    std::fs::write(store.path(&hash), b"tampered").unwrap();
    assert!(store.read(&hash).is_err(), "bytes that do not hash to their name are not served");
    assert!(store.read(&model::Hash::of(b"never stored")).is_err());
    let read = Folder::new(site("into-store", &[("a.md", "a")])).read(&mut store).unwrap();
    assert_eq!(store.read(&read.snapshot.tree()[0].blob).unwrap().as_ref(), b"a");
}
