//! Hostile input: no bytes panic, the same input reads the same, and every span lies inside the
//! file, across the syntax Obsidian adds: wikilinks, embeds, callouts, block ids, tags, comments
//! and aliases.

#![expect(clippy::unwrap_used, clippy::indexing_slicing, reason = "tests may unwrap")]

use model::{ContentPath, Read};
use proptest::prelude::*;

fn read(bytes: &[u8]) -> Read {
    uses_ink_format_obsidian::read(&ContentPath::parse("note.md").unwrap(), bytes)
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

/// The pieces an Obsidian note is made of: the characters that start its syntax, and the markdown
/// they mix with.
const PIECES: &[&str] = &[
    "[[",
    "]]",
    "![[",
    "[[Note]]",
    "![[Note]]",
    "![[Note#Heading]]",
    "![[Note#^block]]",
    "[[Note#A#B|text]]",
    "[[#Heading]]",
    "![[photo.png|300x200]]",
    "![[slides.pdf]]",
    "|",
    "#",
    "# ",
    "#tag",
    "#a/b",
    "#2024",
    "^id",
    " ^id\n",
    "\n^id\n",
    "^",
    "%%",
    "%%comment%%",
    "> [!note]",
    "> [!tip]- Title",
    "> [!x|meta]+\n",
    "> ",
    "---\n",
    "aliases: [A, B]\n",
    "alias: One\n",
    "tags: [a, b]\n",
    "title: T\n",
    "~",
    "~x~",
    "~~",
    "~~x~~",
    "`",
    "`~x~`",
    "```\n",
    "**",
    "*",
    "==",
    "$",
    "$$",
    "\n",
    "\n\n",
    "- ",
    "| a | b |\n|---|---|\n",
    "<kbd>",
    "<!--",
    "-->",
    "é",
    "\r",
    "\t",
    " ",
    "a",
];

fn obsidian() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(PIECES), 0..80).prop_map(|pieces| pieces.concat())
}

proptest! {
    // Seeded, so a failure reproduces; proptest prints the failing input.
    #![proptest_config(ProptestConfig {
        rng_seed: proptest::test_runner::RngSeed::Fixed(0x0b51_d1a4),
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
    fn notes_never_panic(text in obsidian()) {
        let first = read(text.as_bytes());
        prop_assert_eq!(&first, &read(text.as_bytes()));
        spans_inside(&first, text.len());
    }

    /// Code is code: a single-tilde pair in a code span reads as the code it is, however it is
    /// surrounded.
    #[test]
    fn code_spans_keep_their_tildes(before in "[a-z]{0,8}", after in "[a-z]{0,8}") {
        let text = format!("{before} `~x~` {after}");
        let read = read(text.as_bytes());
        let json = serde_json::to_string(&read.document).unwrap();
        prop_assert!(json.contains(r#"{"code":"~x~"}"#), "{json}");
    }
}
