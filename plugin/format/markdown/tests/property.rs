//! Hostile input: no bytes panic, the same input reads the same, every span lies inside the file,
//! and nesting deeper than the walk allows keeps its text.

#![expect(clippy::unwrap_used, clippy::indexing_slicing, reason = "tests may unwrap")]

use model::{ContentPath, Read};
use proptest::prelude::*;

fn read(bytes: &[u8]) -> Read {
    uses_ink_format_markdown::read(&ContentPath::parse("post.md").unwrap(), bytes)
}

fn spans_inside(read: &Read, len: usize) {
    let json = serde_json::to_value(read).unwrap();
    let mut stack = vec![&json];
    while let Some(value) = stack.pop() {
        match value {
            serde_json::Value::Object(map) => {
                for (key, value) in map {
                    if matches!(key.as_str(), "span" | "block")
                        && let Some(pair) = value.as_array()
                    {
                        let end = usize::try_from(pair[1].as_u64().unwrap()).unwrap();
                        assert!(end <= len, "{key} ends at {end}, past {len}");
                    }
                    stack.push(value);
                }
            }
            serde_json::Value::Array(items) => stack.extend(items),
            _ => {}
        }
    }
}

/// The pieces markdown-ish text is made of: the characters that start syntax.
const PIECES: &[&str] = &[
    "#",
    "# ",
    "---\n",
    "\n",
    "\n\n",
    "> ",
    "> [!note]\n",
    "- ",
    "1. ",
    "*",
    "**",
    "~~",
    "`",
    "```\n",
    "$",
    "$$",
    "|",
    "| --- |",
    "[",
    "]",
    "(",
    ")",
    "![",
    "<",
    ">",
    "</",
    "div",
    "script",
    "kbd",
    "details",
    "summary",
    "a href=",
    "==",
    "^",
    "~",
    "<!--",
    "-->",
    "&amp;",
    "title: ",
    "tags: [a, b]",
    "date: 2024-03-01",
    ": ",
    "é",
    "\r",
    "\t",
    "    ",
    "https://example.org",
    "#x",
    "a",
    " ",
];

fn markdownish() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(PIECES), 0..80).prop_map(|pieces| pieces.concat())
}

proptest! {
    // Seeded, so a failure reproduces; proptest prints the failing input.
    #![proptest_config(ProptestConfig {
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x1a4b_2c3d),
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..400)) {
        let first = read(&bytes);
        prop_assert_eq!(&first, &read(&bytes));
        spans_inside(&first, bytes.len());
    }

    #[test]
    fn markdown_never_panics(text in markdownish()) {
        let first = read(text.as_bytes());
        prop_assert_eq!(&first, &read(text.as_bytes()));
        spans_inside(&first, text.len());
    }
}

#[test]
fn deep_nesting_keeps_text() {
    for open in [">", "- ", "*", "["] {
        let text = open.repeat(20_000) + "deep";
        let read = read(text.as_bytes());
        spans_inside(&read, text.len());
        let nested = read.diagnostics.iter().any(|d| d.code() == "page.nesting.deep");
        // comrak itself stops nesting lists at its own limit, below ours
        assert_eq!(nested, open == ">", "{open}");
    }
}
