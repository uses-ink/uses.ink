//! The adapter against the port's conformance suite, each test in its own throwaway folder.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_blob_dir as blob_dir;

/// A store in a folder that is removed when the test ends.
fn fresh() -> (blob_dir::Directory, tempfile::TempDir) {
    let folder = tempfile::tempdir().unwrap();
    (blob_dir::Directory::open(folder.path()).unwrap(), folder)
}

blob_conformance::suite!(fresh);
