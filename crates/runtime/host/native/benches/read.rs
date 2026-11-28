//! The folder walk and the git read on large inputs: a generated tree of 20,000 files, and the
//! repository `INK_BENCH_REPO` names (tldr in decision 0036's measurements), skipped and said
//! when it is not set. `cargo bench -p uses-ink-host-native`.

#![allow(clippy::unwrap_used, clippy::print_stderr, reason = "a bench stops at the first surprise")]

extern crate uses_ink_host_native as host_native;

use std::path::{Path, PathBuf};

use divan::Bencher;
use host_native::{Budget, Folder, Memory, Repository};

fn main() {
    divan::main();
}

/// 200 folders of 100 notes of about a kilobyte each, made once under target/.
fn wide() -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("bench-wide");
    // beside the tree, which holds exactly the 20,000 files the v1 budget allows
    let done = root.with_extension("made");
    if done.is_file() {
        return root;
    }
    for folder in 0..200 {
        let dir = root.join(format!("section{folder:03}"));
        std::fs::create_dir_all(&dir).unwrap();
        for note in 0..100 {
            let text = format!("# Note {folder} {note}\n\n{}\n", "Some text about things. ".repeat(40));
            std::fs::write(dir.join(format!("note{note:03}.md")), text).unwrap();
        }
    }
    std::fs::write(done, "").unwrap();
    root
}

#[divan::bench(sample_count = 10, sample_size = 1)]
fn folder(bencher: Bencher<'_, '_>) {
    let folder = Folder::new(wide());
    bencher.bench(|| folder.read(&mut Memory::default()).unwrap());
}

#[divan::bench(sample_count = 3, sample_size = 1)]
fn git(bencher: Bencher<'_, '_>) {
    let Some(repo) = std::env::var_os("INK_BENCH_REPO") else {
        eprintln!("git: skipped, INK_BENCH_REPO names no repository");
        return;
    };
    // tldr holds twice the v1 budget's files
    let budget = Budget { files: 1_000_000, folders: 1_000_000, ..Budget::default() };
    let repository = Repository { budget, ..Repository::new(PathBuf::from(repo)) };
    bencher.bench(|| repository.read(&mut Memory::default()).unwrap());
}
