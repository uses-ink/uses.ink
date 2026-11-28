//! The text the docs checks read: every path of the tree, and the text of every file that can
//! hold a prose or code citation. Read once, so each check is a function of this value.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use super::decision::Decisions;
use super::number::Number;
use crate::error::Error;
use crate::layout::{Config, Workspace};

/// A text file: its path relative to the root, and its text.
#[derive(Debug)]
pub(crate) struct Doc {
    pub(crate) path: PathBuf,
    pub(crate) text: String,
}

/// One line of a document's prose, outside code fences.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Row<'a> {
    /// Counted from 1.
    pub(crate) line: usize,
    pub(crate) text: &'a str,
}

impl Row<'_> {
    /// Whether it is a block of its own line: a heading or a table row.
    fn single(&self) -> bool {
        let text = self.text.trim_start();
        text.starts_with('#') || text.starts_with('|')
    }

    /// Whether it starts a new block: a list item, a heading or a table row.
    fn opens(&self) -> bool {
        let text = self.text.trim_start();
        ["- ", "* ", "#", "|"].iter().any(|mark| text.starts_with(mark))
            || text
                .split_once(". ")
                .is_some_and(|(count, _)| !count.is_empty() && count.bytes().all(|b| b.is_ascii_digit()))
    }
}

impl Doc {
    /// Whether it is Markdown the repository's readers open first: the docs, the README and the
    /// product brief.
    pub(crate) fn is_prose(&self) -> bool {
        self.path.extension().is_some_and(|extension| extension == "md")
            && (self.path.starts_with("docs")
                || self.path == Path::new("README.md")
                || self.path == Path::new("PRODUCT.md"))
    }

    /// Whether it is Markdown under `docs/`.
    pub(crate) fn is_doc(&self) -> bool {
        self.is_prose() && self.path.starts_with("docs")
    }

    /// Its lines outside fenced code blocks.
    pub(crate) fn rows(&self) -> Vec<Row<'_>> {
        let mut fenced = false;
        self.text
            .lines()
            .enumerate()
            .filter_map(|(index, text)| {
                let trimmed = text.trim_start();
                if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                    fenced = !fenced;
                    return None;
                }
                (!fenced).then_some(Row { line: index + 1, text })
            })
            .collect()
    }

    /// Its rows in blocks: a paragraph, a list item, a heading or a table row each.
    pub(crate) fn blocks(&self) -> Vec<Vec<Row<'_>>> {
        let mut blocks = Vec::new();
        let mut block: Vec<Row<'_>> = Vec::new();
        for row in self.rows() {
            let blank = row.text.trim().is_empty();
            let breaks = blank || row.opens() || block.first().is_some_and(Row::single);
            if breaks && !block.is_empty() {
                blocks.push(std::mem::take(&mut block));
            }
            if !blank {
                block.push(row);
            }
        }
        if !block.is_empty() {
            blocks.push(block);
        }
        blocks
    }
}

/// Every path of the tree and the text files among them.
#[derive(Debug)]
pub(crate) struct Corpus {
    /// Every path relative to the root, files and directories.
    pub(crate) entries: BTreeSet<PathBuf>,
    /// Every `.rs`, `.toml`, `.yml` and `.md` file.
    pub(crate) files: Vec<Doc>,
    pub(crate) config: Config,
}

impl Corpus {
    /// The extensions of the files a citation may be in.
    const EXTENSIONS: [&'static str; 4] = ["rs", "toml", "yml", "md"];

    /// Reads the tree under `root`. A file that is not UTF-8 is read lossily: a citation is ASCII.
    pub(crate) fn read(root: &Path) -> Result<Self, Error> {
        let config = Config::read(root)?;
        let mut paths = Vec::new();
        Workspace::walk(root, root, &mut paths)?;
        let mut files = Vec::new();
        for path in &paths {
            let text =
                path.extension().is_some_and(|extension| Self::EXTENSIONS.iter().any(|known| extension == *known));
            let full = root.join(path);
            if !text || !full.is_file() {
                continue;
            }
            let bytes = std::fs::read(&full).map_err(|source| Error::Read { path: full, source })?;
            files.push(Doc { path: path.clone(), text: String::from_utf8_lossy(&bytes).into_owned() });
        }
        Ok(Self { entries: paths.into_iter().collect(), files, config })
    }

    /// The text of the file at `path`, if it is one of [`Corpus::files`].
    pub(crate) fn text(&self, path: &Path) -> Option<&str> {
        self.files.iter().find(|doc| doc.path == path).map(|doc| doc.text.as_str())
    }

    /// The path a link `target` written in a file of directory `dir` names, if it stays in the
    /// tree. The `..` and `.` parts are resolved by their text.
    pub(crate) fn resolve(dir: &Path, target: &str) -> Option<PathBuf> {
        let mut path = PathBuf::new();
        if !target.starts_with('/') {
            path.push(dir);
        }
        for part in Path::new(target).components() {
            match part {
                Component::Normal(name) => path.push(name),
                Component::ParentDir if path.pop() => {}
                Component::ParentDir => return None,
                Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
            }
        }
        Some(path)
    }

    /// Whether a link `target` written in a file of directory `dir` names a path of the tree.
    pub(crate) fn reaches(&self, dir: &Path, target: &str) -> bool {
        Self::resolve(dir, target).is_some_and(|path| path.as_os_str().is_empty() || self.entries.contains(&path))
    }

    /// The first part of every path that has a second: the directories at the root.
    pub(crate) fn roots(&self) -> BTreeSet<String> {
        self.entries
            .iter()
            .filter(|entry| entry.components().count() > 1)
            .filter_map(|entry| entry.components().next())
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect()
    }

    /// The number of every decision that has a file in `docs/decisions/`.
    pub(crate) fn filed(&self) -> BTreeSet<Number> {
        self.entries
            .iter()
            .filter(|entry| entry.parent() == Some(Path::new(Decisions::DIR)))
            .filter_map(|entry| entry.file_name()?.to_str()?.get(..Number::WIDTH))
            .filter_map(|text| Number::parse(text).ok())
            .collect()
    }

    /// The number of every decision that exists: filed, or reserved.
    pub(crate) fn numbers(&self) -> BTreeSet<Number> {
        let reserved = self.config.decisions.reserved.iter().map(|reserved| reserved.number);
        self.filed().into_iter().chain(reserved).collect()
    }
}

#[cfg(test)]
impl Corpus {
    /// A tree of `files`, each `(path, text)`, with the ceiling the real file sets.
    pub(crate) fn sample(files: &[(&str, &str)]) -> Self {
        let mut config = Config::default();
        config.decisions.ceiling = 99;
        let mut corpus = Self { entries: BTreeSet::new(), files: Vec::new(), config };
        for (path, text) in files {
            corpus.files.push(Doc { path: PathBuf::from(path), text: (*text).to_owned() });
            corpus = corpus.also(path);
        }
        corpus
    }

    /// Adds `path` and every directory above it to the tree.
    pub(crate) fn also(mut self, path: &str) -> Self {
        let mut at = Some(Path::new(path));
        while let Some(current) = at.filter(|current| !current.as_os_str().is_empty()) {
            self.entries.insert(current.to_path_buf());
            at = current.parent();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_resolve_from_the_directory_of_the_file() {
        let corpus = Corpus::sample(&[("docs/a.md", ""), ("docs/decisions/b.md", ""), ("README.md", "")]);
        assert!(corpus.reaches(Path::new("docs"), "a.md"));
        assert!(corpus.reaches(Path::new("docs"), "./decisions/b.md"));
        assert!(corpus.reaches(Path::new("docs/decisions"), "../a.md"));
        assert!(corpus.reaches(Path::new("docs/decisions"), "../../README.md"));
        assert!(corpus.reaches(Path::new("docs"), "decisions/"));
        assert!(corpus.reaches(Path::new("docs"), "/README.md"));
        assert!(!corpus.reaches(Path::new("docs"), "nope.md"));
        assert!(!corpus.reaches(Path::new("docs"), "../../README.md"));
    }

    #[test]
    fn fenced_code_is_not_prose() {
        let doc = Doc { path: PathBuf::from("a.md"), text: "one\n```\ntwo\n```\nthree\n".to_owned() };
        let lines: Vec<(usize, &str)> = doc.rows().iter().map(|row| (row.line, row.text)).collect();
        assert_eq!(lines, [(1, "one"), (5, "three")]);
    }

    #[test]
    fn rows_split_into_blocks_at_blanks_items_and_headings() {
        let doc = Doc {
            path: PathBuf::from("a.md"),
            text: "# Head\npara one\ncontinues\n\n- item\n  wraps\n- next\n1. first\n".to_owned(),
        };
        let blocks: Vec<Vec<&str>> =
            doc.blocks().iter().map(|block| block.iter().map(|row| row.text).collect()).collect();
        assert_eq!(
            blocks,
            [
                vec!["# Head"],
                vec!["para one", "continues"],
                vec!["- item", "  wraps"],
                vec!["- next"],
                vec!["1. first"]
            ]
        );
    }

    #[test]
    fn roots_are_the_directories_at_the_top() {
        let corpus = Corpus::sample(&[("a/b.rs", ""), ("README.md", "")]).also("c/d/e");
        assert_eq!(corpus.roots().into_iter().collect::<Vec<_>>(), ["a", "c"]);
    }
}
