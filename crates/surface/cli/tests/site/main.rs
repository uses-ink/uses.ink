//! `ink` on every folder under `fixture/site/`, in process: each build compared with
//! `fixture/site/<name>.snap` (the exit code, the diagnostics, the manifest and the HTML of every
//! page), `ink explain` on each compared with `fixture/explain/<name>.snap`, and the M1 exit
//! tests: a build twice is the same deploy, a pin of a decision's current value builds the same
//! deploy again, and no reader page runs script.
//!
//! The blog has code, so tests that build it compile a tree-sitter grammar and carry `highlight`
//! in their name for its budget (`.config/nextest.toml`).

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "tests stop at the first surprise"
)]

extern crate uses_ink_cli as cli;

mod build;
mod check;
mod explain;
mod format;
mod pin;

use std::path::{Path, PathBuf};

use clap::Parser;
use cli::{Cli, Exit};

const BASE: &str = "https://ink.example";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site")
}

/// A fresh scratch folder for `name`.
fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli").join(name);
    if path.exists() {
        std::fs::remove_dir_all(&path).unwrap();
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    path
}

/// The cache every build here shares, under target/ so it outlives the run and never lands in
/// a fixture: after the first run the blog's photos are taken from it, not encoded.
fn cache() -> String {
    Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-cache").join("site").to_str().unwrap().to_owned()
}

/// `ink <args>`: the exit, stdout and stderr.
fn ink(args: &[&str]) -> (Exit, String, String) {
    let cli = Cli::try_parse_from(std::iter::once("ink").chain(args.iter().copied())).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let exit = cli.run(&mut out, &mut err);
    (exit, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// Builds `dir` into `out` and returns the deploy id.
fn build(dir: &Path, out: &Path) -> String {
    let (exit, stdout, stderr) =
        ink(&["build", "-C", dir.to_str().unwrap(), "-o", out.to_str().unwrap(), "--base", BASE, "--cache", &cache()]);
    assert_eq!(exit, Exit::Done, "{stderr}");
    stdout.trim().to_owned()
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() { copy(&path, &target) } else { std::fs::copy(&path, &target).map(|_| ()).unwrap() }
    }
}

/// The name of every site under `fixture/site/`, sorted.
fn sites() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixtures())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir())
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_owned())
        .collect();
    names.sort();
    assert!(names.len() >= 6, "the fixtures are missing: {names:?}");
    names
}
