//! The harness fails loudly: each way a suite can be wrong, on a directory of its own.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

use std::path::PathBuf;

use uses_ink_fixture as fixture;

/// A fresh directory holding `files`, as (name, contents).
fn dir(test: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fixture").join(test);
    let _absent = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, contents) in files {
        std::fs::write(dir.join(name), contents).unwrap();
    }
    dir
}

/// A golden as insta writes it.
const A: &str = "---\nsource: fixture/src/lib.rs\n---\nA\n";

fn upper(_: &std::path::Path, bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_ascii_uppercase()).unwrap()
}

#[test]
fn a_matching_suite_passes() {
    fixture::files(dir("matching", &[("a.txt", "a"), ("a.snap", A), ("notes.md", "not an input")]), "txt", upper);
}

#[test]
fn cases_match_their_goldens() {
    fixture::cases(dir("cases", &[("a.snap", A)]), [("a", "A".to_owned())]);
}

#[test]
#[should_panic(expected = "no inputs, so nothing was compared")]
fn an_empty_suite_fails() {
    fixture::files(dir("empty", &[]), "txt", upper);
}

#[test]
#[should_panic(expected = "no golden")]
fn an_input_without_a_golden_fails() {
    fixture::files(dir("missing", &[("a.txt", "a")]), "txt", upper);
}

#[test]
#[should_panic(expected = "a golden without an input")]
fn a_golden_without_an_input_fails() {
    fixture::files(dir("orphan", &[("a.txt", "a"), ("a.snap", A), ("b.snap", A)]), "txt", upper);
}

#[test]
#[should_panic(expected = "an update waiting for review")]
fn a_pending_update_fails() {
    fixture::files(dir("pending", &[("a.txt", "a"), ("a.snap", A), ("b.snap.new", A)]), "txt", upper);
}

#[test]
#[should_panic(expected = "the output differs")]
fn a_changed_output_fails() {
    fixture::files(dir("differs", &[("a.txt", "b"), ("a.snap", A)]), "txt", upper);
}

#[test]
#[should_panic(expected = "two outputs share the name")]
fn two_cases_with_one_name_fail() {
    fixture::cases(dir("twice", &[("a.snap", A)]), [("a", "A".to_owned()), ("a", "A".to_owned())]);
}

/// Run alone by `an_update_writes_and_fails`, in a child with `INSTA_UPDATE=always`.
#[test]
#[ignore = "run by an_update_writes_and_fails with INSTA_UPDATE set"]
fn update_child() {
    fixture::files(dir("update", &[("a.txt", "a")]), "txt", upper);
}

#[test]
fn an_update_writes_and_fails() {
    for mode in ["always", "new"] {
        let child = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["update_child", "--exact", "--ignored"])
            .env("INSTA_UPDATE", mode)
            .output()
            .unwrap();
        let said = String::from_utf8_lossy(&child.stdout) + String::from_utf8_lossy(&child.stderr);
        assert!(!child.status.success(), "{mode}: an update passed");
        assert!(said.contains("a.snap: written in update mode"), "{mode}: {said}");
    }
}
