//! The local builder against the port's conformance suite, over a folder with one page.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_build_local as build_local;

use blob_dir::Directory;
use build::Source;
use build_conformance::Harness;
use build_local::Local;

/// A site of one page, a site whose `ink.toml` has a key nothing reads, and the store the
/// builder writes to, with a second handle on it.
fn fresh() -> (Harness<Local<Directory>, Directory>, tempfile::TempDir) {
    let folder = tempfile::tempdir().unwrap();
    let good = folder.path().join("good");
    let broken = folder.path().join("broken");
    for dir in [&good, &broken] {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("index.md"), "# Hello\n\nOne page, no code and no images.\n").unwrap();
    }
    std::fs::write(broken.join("ink.toml"), "[site]\nno-such-key = 1\n").unwrap();
    let store = folder.path().join("blobs");
    let harness = Harness {
        builder: Local::new(Directory::open(&store).unwrap()),
        blobs: Directory::open(&store).unwrap(),
        good: Source::Folder(good),
        broken: Source::Folder(broken),
    };
    (harness, folder)
}

build_conformance::suite!(fresh);
