//! The numbers decision 0054 cites for bundling: the binary's size and what starting it costs.
//! Run with `cargo test --profile dist -p uses-ink-cli --test measure -- --ignored --nocapture`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stdout,
    reason = "a measurement stops at the first surprise"
)]

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const INK: &str = env!("CARGO_BIN_EXE_ink");

/// The median of `rounds` runs of `work`.
fn median(rounds: usize, mut work: impl FnMut()) -> Duration {
    let mut times: Vec<Duration> = (0..rounds)
        .map(|_| {
            let started = Instant::now();
            work();
            started.elapsed()
        })
        .collect();
    times.sort();
    times[rounds / 2]
}

fn run(args: &[&str]) {
    let status = Command::new(INK).args(args).stdout(Stdio::null()).stderr(Stdio::null()).status().unwrap();
    assert!(status.success() || args.contains(&"build"), "{args:?}");
}

#[test]
#[ignore = "a measurement, not a check: run it by hand with the dist profile"]
fn measure() {
    let size = std::fs::metadata(INK).unwrap().len();
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/plugin");
    let carried: u64 = ["format-markdown", "format-obsidian", "theme-quiet", "source-git"]
        .iter()
        .filter_map(|name| std::fs::metadata(plugins.join(format!("{name}.wasm"))).ok())
        .map(|meta| meta.len())
        .sum();
    println!("ink is {size} bytes; its components are {carried} bytes of that");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixture/site/blog");
    let fixture = fixture.to_str().unwrap();
    println!("ink --version, median of 400: {:?}", median(400, || run(&["--version"])));
    println!("ink check <blog>, median of 200: {:?}", median(200, || run(&["check", "-C", fixture])));
}
