//! How long reading one file takes: a post of about 2,000 words, the 1.6 MB flood of 20,000
//! paragraphs, 5,000 headings of the same text and 5,000 `<details>` never closed. `cargo bench
//! -p uses-ink-format-markdown`.

#![allow(clippy::unwrap_used, reason = "a bench stops at the first surprise")]

use divan::{Bencher, black_box};
use fixture::corpus::{Flood, Post, Repeated};
use model::ContentPath;

fn main() {
    divan::main();
}

#[divan::bench]
fn post(bencher: Bencher<'_, '_>) {
    let path = ContentPath::parse("posts/post.md").unwrap();
    let text = Post { seed: 1, links: 0 }.to_string();
    bencher.bench(|| uses_ink_format_markdown::read(&path, black_box(text.as_bytes())));
}

#[divan::bench(sample_count = 10)]
fn flood(bencher: Bencher<'_, '_>) {
    let path = ContentPath::parse("flood.md").unwrap();
    let text = Flood { paragraphs: 20_000 }.to_string();
    bencher.bench(|| uses_ink_format_markdown::read(&path, black_box(text.as_bytes())));
}

/// Every heading's anchor taken by the one before: `intro`, `intro-1`, `intro-2`.
#[divan::bench(sample_count = 10)]
fn headings(bencher: Bencher<'_, '_>) {
    let path = ContentPath::parse("headings.md").unwrap();
    let text = Repeated { block: "## Intro\n\ntext\n\n", times: 5_000 }.to_string();
    bencher.bench(|| uses_ink_format_markdown::read(&path, black_box(text.as_bytes())));
}

/// Each `<details>` inside the one before, none closed.
#[divan::bench(sample_count = 10)]
fn folds(bencher: Bencher<'_, '_>) {
    let path = ContentPath::parse("folds.md").unwrap();
    let text = Repeated { block: "<details>\n\ntext\n\n", times: 5_000 }.to_string();
    bencher.bench(|| uses_ink_format_markdown::read(&path, black_box(text.as_bytes())));
}
