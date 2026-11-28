//! Code highlighted through the built-in grammars: every enabled language captures something,
//! the runs cover the code exactly, and a language no grammar knows is said to be unknown.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, reason = "tests stop at the first surprise")]

extern crate uses_ink_host_native as host_native;

use host_native::Grammars;
use model::highlight::{Capture, Highlight, Run, Unhighlighted};

/// The runs of `code` in `language`, checked to cover it in order with no empty run and no two
/// neighbours of one capture.
fn runs<'c>(language: &str, code: &'c str) -> Vec<Run<'c>> {
    let runs = Grammars::default().highlight(language, code).unwrap();
    assert_eq!(runs.iter().map(|run| run.text).collect::<String>(), code, "{language}: the runs are the code");
    assert!(runs.iter().all(|run| !run.text.is_empty()), "{language}: an empty run");
    assert!(runs.windows(2).all(|pair| pair[0].capture != pair[1].capture), "{language}: runs not joined");
    runs
}

/// One test per enabled language, each a snippet in which the grammar must capture `$capture`.
///
/// ```text
/// languages! { rust: "rust", "fn main() {}", Keyword; }
/// // makes `#[test] fn rust()` highlighting that snippet
/// ```
macro_rules! languages {
    ($($test:ident: $language:literal, $code:literal, $capture:ident;)+) => {
        $(
            #[test]
            fn $test() {
                let runs = runs($language, $code);
                assert!(
                    runs.iter().any(|run| run.capture == Some(Capture::$capture)),
                    "{}: no {:?} in {runs:?}", $language, Capture::$capture,
                );
            }
        )+
    };
}

languages! {
    bash: "bash", "echo \"$HOME\" # home\n", Comment;
    c: "c", "int main(void) { return 0; }\n", Keyword;
    css: "css", ".nib { color: red; }\n", Property;
    // the grammar captures additions as strings and deletions as keywords
    diff: "diff", "--- a\n+++ b\n-old\n+new\n", String;
    go: "go", "package main\nfunc main() {}\n", Keyword;
    html: "html", "<p class=\"x\">hi</p>\n", Tag;
    javascript: "js", "const x = 1; // one\n", Comment;
    json: "json", "{\"a\": 1}\n", Number;
    markdown: "markdown", "# Title\n\nText.\n", Heading;
    python: "python", "def f():\n    return 'x'\n", String;
    rust: "rust", "fn main() { println!(\"hé\"); }\n", Macro;
    toml: "toml", "[site]\ntitle = \"Ink\"\n", String;
    typescript: "ts", "let n: number = 1;\n", Type;
    yaml: "yml", "title: Ink\n", Property;
}

/// A macro's arguments are parsed again as an injection, which captures an empty span after
/// them; nothing after it takes its capture.
#[test]
fn an_empty_span_styles_nothing() {
    let runs = runs("rust", "fn main() {\n    println!(\"x\");\n}\n");
    let tail: Vec<Run<'_>> = runs.iter().rev().take(4).rev().copied().collect();
    let punctuation = Some(Capture::Punctuation);
    assert_eq!(
        tail,
        [
            Run { capture: punctuation, text: ");" },
            Run { capture: None, text: "\n" },
            Run { capture: punctuation, text: "}" },
            Run { capture: None, text: "\n" },
        ],
        "{runs:?}",
    );
}

#[test]
fn names_match_without_case() {
    let runs = runs("Rust", "fn main() {}\n");
    assert_eq!(runs.first(), Some(&Run { capture: Some(Capture::Keyword), text: "fn" }));
}

#[test]
fn an_unknown_language_is_said_to_be() {
    let refused = Grammars::default().highlight("no-such-language", "x\n");
    assert!(matches!(refused, Err(Unhighlighted::Unknown)), "{refused:?}");
}

#[test]
fn empty_code_is_no_runs() {
    assert_eq!(runs("rust", ""), []);
}
