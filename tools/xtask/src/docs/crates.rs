//! Every backticked path of a crate or directory in `docs/` and in a crate's README exists, unless its block (a
//! paragraph, list item, heading or table row) says it is planned. A path is one that has a `/`
//! and starts with a directory of the root; globs, placeholders, URLs and `..` parts are not.

use std::collections::BTreeSet;
use std::path::Path;

use super::Check;
use super::corpus::Corpus;
use super::fault::Fault;

/// The crate path check.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Crates;

impl Crates {
    /// Characters that make a backticked span a pattern, a placeholder or a URL, not a path.
    const PATTERN: &'static str = "*<>{}$|~:\\\"'()[]";

    /// The path `span` names, with its `:line` suffix and trailing `/` dropped, when it looks
    /// like one under a directory of `roots`.
    fn candidate(span: &str, roots: &BTreeSet<String>) -> Option<String> {
        let lined = span.rsplit_once(':').filter(|(_, tail)| {
            tail.split('-').all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        });
        let path = lined.map_or(span, |(head, _)| head);
        let plain = !path.contains(|found: char| found.is_whitespace() || Self::PATTERN.contains(found))
            && !path.starts_with(['/', '.', '-'])
            && !path.contains("..");
        if !plain {
            return None;
        }
        let path = path.trim_end_matches('/');
        let (first, _) = path.split_once('/')?;
        roots.contains(first).then(|| path.to_owned())
    }
}

impl Check for Crates {
    fn check(&self, corpus: &Corpus, faults: &mut Vec<Fault>) {
        let roots = corpus.roots();
        let marks: Vec<String> = corpus.config.docs.planned.iter().map(|mark| mark.to_lowercase()).collect();
        let docs = corpus
            .files
            .iter()
            .filter(|doc| doc.is_doc())
            .filter(|doc| !corpus.config.docs.future.iter().any(|future| doc.path.starts_with(&future.path)));
        for doc in docs {
            for block in doc.blocks() {
                let planned = block.iter().any(|row| {
                    let text = row.text.to_lowercase();
                    marks.iter().any(|mark| text.contains(mark.as_str()))
                });
                if planned {
                    continue;
                }
                for row in &block {
                    for span in row.text.split('`').skip(1).step_by(2) {
                        let Some(path) = Self::candidate(span, &roots) else { continue };
                        if !corpus.entries.contains(Path::new(&path)) {
                            faults.push(Fault::Missing { file: doc.path.clone(), line: row.line, path });
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn corpus(text: &str) -> Corpus {
        let mut corpus =
            Corpus::sample(&[("docs/a.md", text), ("edge/api/src/lib.rs", ""), ("db/sqlite/Cargo.toml", "")]);
        corpus.config.docs.planned = vec!["planned".to_owned(), "Later".to_owned()];
        corpus
    }

    fn found(corpus: &Corpus) -> Vec<String> {
        let mut faults = Vec::new();
        Crates.check(corpus, &mut faults);
        faults.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn an_existing_path_passes_with_a_line_suffix_a_slash_or_a_file() {
        let corpus = corpus("`edge/api` and `edge/api/src/lib.rs:12-14` and `db/sqlite/` and `db/sqlite/Cargo.toml`\n");
        assert_eq!(found(&corpus), Vec::<String>::new());
    }

    #[test]
    fn a_missing_path_is_a_fault_with_its_line() {
        let corpus = corpus("fine\nthe `edge/worker` crate and `db/sqlite/src`\n");
        assert_eq!(
            found(&corpus),
            [
                "docs/a.md:2: `edge/worker` does not exist; mark the sentence planned, or fix the path",
                "docs/a.md:2: `db/sqlite/src` does not exist; mark the sentence planned, or fix the path"
            ]
        );
    }

    #[test]
    fn patterns_urls_and_paths_outside_the_roots_are_left() {
        let corpus =
            corpus("`edge/*` `edge/<slot>/x` `https://edge/x` `elsewhere/x` `../edge/x` `~/edge/x` `edge` `a b/c`\n");
        assert_eq!(found(&corpus), Vec::<String>::new());
    }

    #[test]
    fn a_block_with_a_planned_marker_is_left_and_the_next_block_is_not() {
        let text = "- `edge/worker` is planned\n- `edge/other` is real\n\nThe `db/d1` adapter comes Later.\n";
        assert_eq!(
            found(&corpus(text)),
            ["docs/a.md:2: `edge/other` does not exist; mark the sentence planned, or fix the path"]
        );
    }

    #[test]
    fn a_file_of_the_future_list_and_fenced_code_are_left() {
        let mut corpus = corpus("```\n`edge/worker`\n```\n");
        assert_eq!(found(&corpus), Vec::<String>::new());
        corpus.files.push(crate::docs::corpus::Doc {
            path: std::path::PathBuf::from("docs/plan.md"),
            text: "`edge/worker`\n".to_owned(),
        });
        assert_eq!(found(&corpus).len(), 1);
        corpus.config.docs.future = vec![crate::layout::config::Future::sample("docs/plan.md")];
        assert_eq!(found(&corpus), Vec::<String>::new());
    }

    #[test]
    fn only_markdown_under_docs_and_crate_readmes_is_read() {
        let mut corpus = Corpus::sample(&[("README.md", "`edge/worker`\n"), ("edge/a.rs", "// `edge/worker`\n")]);
        corpus.config.docs.planned = vec![];
        assert_eq!(found(&corpus), Vec::<String>::new());
        let mut corpus = Corpus::sample(&[
            ("crates/edge/README.md", "`edge/worker`\n"),
            ("crates/edge/fixture/site/README.md", "`edge/worker`\n"),
            ("edge/a.rs", ""),
        ]);
        corpus.config.docs.planned = vec![];
        assert_eq!(found(&corpus).len(), 1);
    }
}
