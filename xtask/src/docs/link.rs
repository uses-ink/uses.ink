//! Every relative Markdown link of the docs, the README and the product brief names a path of the
//! tree. Anchors are not checked: only the path before `#`.

use std::path::Path;

use super::Check;
use super::corpus::Corpus;
use super::fault::Fault;

/// The link check.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Links;

impl Links {
    /// `text` with the inside of each inline code span blanked, so a link written as an example
    /// is not one.
    fn plain(text: &str) -> String {
        let mut code = false;
        text.chars()
            .map(|found| match found {
                '`' => {
                    code = !code;
                    ' '
                }
                _ if code => ' ',
                other => other,
            })
            .collect()
    }

    /// The targets of the inline links and the reference definition on one line.
    fn targets(row: &str) -> Vec<String> {
        let text = Self::plain(row);
        let mut found = Vec::new();
        let definition = text.trim_start().strip_prefix('[').and_then(|rest| rest.split_once("]:"));
        if let Some(target) = definition.and_then(|(_, tail)| tail.split_whitespace().next()) {
            found.push(target.trim_matches(['<', '>']).to_owned());
        }
        for (at, _) in text.match_indices("](") {
            let opened = text.get(..at).is_some_and(|before| before.contains('['));
            let after = text.get(at + 2..).unwrap_or_default();
            let end = after.find(|found: char| found == ')' || found.is_whitespace()).unwrap_or(after.len());
            let target = after.get(..end).unwrap_or_default().trim_matches(['<', '>']);
            if opened && !target.is_empty() {
                found.push(target.to_owned());
            }
        }
        found
    }

    /// Whether `target` starts with a URL scheme, like `https:` or `mailto:`.
    fn scheme(target: &str) -> bool {
        target.split_once(':').is_some_and(|(scheme, _)| {
            scheme.len() > 1
                && scheme.starts_with(|first: char| first.is_ascii_alphabetic())
                && scheme.chars().all(|found| found.is_ascii_alphanumeric() || "+.-".contains(found))
        })
    }

    /// The path part of a target that points into the tree; none for a URL or a bare anchor.
    fn local(target: &str) -> Option<&str> {
        let path = target.split(['#', '?']).next().unwrap_or_default();
        (!path.is_empty() && !Self::scheme(path)).then_some(path)
    }
}

impl Check for Links {
    fn check(&self, corpus: &Corpus, faults: &mut Vec<Fault>) {
        for doc in corpus.files.iter().filter(|doc| doc.is_prose()) {
            let dir = doc.path.parent().unwrap_or(Path::new(""));
            for row in doc.rows() {
                for target in Self::targets(row.text) {
                    if Self::local(&target).is_some_and(|path| !corpus.reaches(dir, path)) {
                        faults.push(Fault::Broken { file: doc.path.clone(), line: row.line, target });
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(files: &[(&str, &str)]) -> Vec<String> {
        let mut faults = Vec::new();
        Links.check(&Corpus::sample(files), &mut faults);
        faults.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_link_that_resolves_passes_with_its_anchor_and_a_url_is_left() {
        let files = [
            ("docs/a.md", "see [b](b.md#part) and [c](sub/c.md) and [web](https://x.dev/y) and [top](#top)\n"),
            ("docs/b.md", ""),
            ("docs/sub/c.md", "[up](../a.md) ![img](../img.png)\n"),
            ("docs/img.png", ""),
        ];
        assert_eq!(found(&files), Vec::<String>::new());
    }

    #[test]
    fn a_link_to_nothing_is_a_fault_with_its_line() {
        let files = [("docs/a.md", "fine\nsee [b](gone.md) here\n"), ("README.md", "[a](docs/missing.md)\n")];
        assert_eq!(
            found(&files),
            [
                "docs/a.md:2: the link to `gone.md` names no path",
                "README.md:1: the link to `docs/missing.md` names no path"
            ]
        );
    }

    #[test]
    fn a_reference_definition_is_a_link() {
        let files = [("docs/a.md", "text [x][1]\n\n[1]: gone.md \"title\"\n")];
        assert_eq!(found(&files), ["docs/a.md:3: the link to `gone.md` names no path"]);
    }

    #[test]
    fn examples_in_code_are_not_links() {
        let files = [("docs/a.md", "write `[x](gone.md)` like this\n```\n[x](gone.md)\n```\n")];
        assert_eq!(found(&files), Vec::<String>::new());
    }

    #[test]
    fn only_prose_files_are_read() {
        let files = [("src/lib.rs", "// [x](gone.md)\n"), ("notes/a.md", "[x](gone.md)\n")];
        assert_eq!(found(&files), Vec::<String>::new());
    }

    #[test]
    fn a_link_above_the_root_names_nothing() {
        assert_eq!(found(&[("README.md", "[x](../out.md)\n")]), ["README.md:1: the link to `../out.md` names no path"]);
    }
}
