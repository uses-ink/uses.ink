//! The globs of a job's `inputs`: which files of the tree a job reads. Small on purpose: `*` and
//! `?` inside one path segment, `**` as a whole segment for any number of them, and a leading
//! `!` to take files back out. A pattern with no `/` matches a name at any depth, as in rsync; a
//! leading `/` or any other `/` anchors it at the root of the tree. A list is read in order and
//! the last pattern that matches a file decides, so `["**", "!docs/**", "docs/workflow.md"]` is
//! everything but docs, except one file. A file no pattern matches is not selected.

use std::fmt;

/// Why a glob is not one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Why {
    Empty,
    /// A segment with nothing in it: `a//b`, or a trailing `/`.
    Segment,
    /// `..` leaves the tree.
    Parent,
    /// `**` mixed with other characters in one segment.
    Double,
    /// `[`, `{`, `\` and the like, which this syntax does not have.
    Syntax {
        found: char,
    },
}

/// A glob that cannot be used, with the text as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fault {
    pub(crate) pattern: String,
    pub(crate) why: Why,
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let pattern = &self.pattern;
        match &self.why {
            Why::Empty => write!(out, "the glob is empty"),
            Why::Segment => write!(out, "`{pattern}` has an empty path segment; write `dir/**` for a directory"),
            Why::Parent => write!(out, "`{pattern}` holds `..`, which leaves the tree"),
            Why::Double => write!(out, "`{pattern}` mixes `**` with other characters; `**` is a whole segment"),
            Why::Syntax { found } => {
                write!(out, "`{pattern}` holds {found:?}; the globs are `*`, `?`, `**` and a leading `!`")
            }
        }
    }
}

impl std::error::Error for Fault {}

/// Whether a pattern takes files in or out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sign {
    In,
    Out,
}

/// One piece of a pattern's path.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    /// `**`.
    Deep,
    /// A segment with `*` and `?` in it.
    Name(Vec<char>),
}

/// One glob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pattern {
    sign: Sign,
    /// Matches from the root of the tree; else any segment of a path may match.
    anchored: bool,
    parts: Vec<Part>,
}

impl Pattern {
    /// Reads a glob as written in the file.
    pub(crate) fn parse(given: &str) -> Result<Self, Fault> {
        let fault = |why| Fault { pattern: given.to_owned(), why };
        let (sign, text) = given.strip_prefix('!').map_or((Sign::In, given), |rest| (Sign::Out, rest));
        if text.is_empty() {
            return Err(fault(Why::Empty));
        }
        if let Some(found) = text.chars().find(|found| matches!(found, '[' | ']' | '{' | '}' | '\\' | '!')) {
            return Err(fault(Why::Syntax { found }));
        }
        let (rooted, body) = text.strip_prefix('/').map_or((false, text), |rest| (true, rest));
        let mut parts = Vec::new();
        for segment in body.split('/') {
            parts.push(match segment {
                "" => return Err(fault(Why::Segment)),
                ".." => return Err(fault(Why::Parent)),
                "**" => Part::Deep,
                other if other.contains("**") => return Err(fault(Why::Double)),
                other => Part::Name(other.chars().collect()),
            });
        }
        Ok(Self { sign, anchored: rooted || parts.len() > 1, parts })
    }

    /// Whether `path` (relative, `/` between segments) is one this pattern matches.
    pub(crate) fn matches(&self, path: &str) -> bool {
        let segments: Vec<&str> = path.split('/').collect();
        if self.anchored {
            return Self::walk(&self.parts, &segments);
        }
        segments.iter().any(|segment| Self::walk(&self.parts, std::slice::from_ref(segment)))
    }

    fn walk(parts: &[Part], segments: &[&str]) -> bool {
        match (parts.split_first(), segments.split_first()) {
            (None, None) => true,
            // a trailing `**` is what is inside a directory: at least one more segment
            (Some((Part::Deep, [])), _) => !segments.is_empty(),
            (Some((Part::Deep, rest)), _) => {
                (0..=segments.len()).any(|skip| segments.get(skip..).is_some_and(|tail| Self::walk(rest, tail)))
            }
            (Some((Part::Name(name), rest)), Some((head, tail))) => {
                Self::fits(name, &head.chars().collect::<Vec<_>>()) && Self::walk(rest, tail)
            }
            (None, Some(_)) | (Some(_), None) => false,
        }
    }

    /// `*` and `?` against one segment.
    fn fits(pattern: &[char], text: &[char]) -> bool {
        match pattern.split_first() {
            None => text.is_empty(),
            Some(('*', rest)) => {
                (0..=text.len()).any(|skip| text.get(skip..).is_some_and(|tail| Self::fits(rest, tail)))
            }
            Some((any, rest)) => {
                text.split_first().is_some_and(|(head, tail)| (*any == '?' || any == head) && Self::fits(rest, tail))
            }
        }
    }
}

/// An ordered list of globs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Selection(Vec<Pattern>);

impl Selection {
    /// Reads the globs of a list; the error is the first bad one with its place in the list.
    pub(crate) fn parse(given: &[String]) -> Result<Self, (usize, Fault)> {
        given
            .iter()
            .enumerate()
            .map(|(at, text)| Pattern::parse(text).map_err(|fault| (at, fault)))
            .collect::<Result<_, _>>()
            .map(Self)
    }

    /// Whether the last pattern that matches `path` takes it in.
    pub(crate) fn selects(&self, path: &str) -> bool {
        self.0.iter().rev().find(|pattern| pattern.matches(path)).is_some_and(|pattern| pattern.sign == Sign::In)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(globs: &[&str]) -> Selection {
        Selection::parse(&globs.iter().map(|glob| (*glob).to_owned()).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn globs_match_by_segment_and_depth() {
        let table = [
            ("**", "a/b/c.rs", true),
            ("docs/**", "docs/a.md", true),
            ("docs/**", "docs/research/a.md", true),
            ("docs/**", "docs", false),
            ("docs/**", "x/docs/a.md", false),
            ("*.md", "docs/a.md", true),
            ("/*.md", "docs/a.md", false),
            ("/*.md", "PRODUCT.md", true),
            ("Cargo.lock", "spikes/x/Cargo.lock", true),
            ("/Cargo.lock", "spikes/x/Cargo.lock", false),
            ("target", "a/target/debug/x", true),
            ("**/src/*.rs", "a/b/src/lib.rs", true),
            ("**/src/*.rs", "src/lib.rs", true),
            ("a?c", "abc", true),
            ("a?c", "ac", false),
            ("freehand/**", "freehand-web/src/lib.rs", false),
        ];
        for (glob, path, want) in table {
            assert_eq!(selection(&[glob]).selects(path), want, "{glob} on {path}");
        }
    }

    #[test]
    fn the_last_match_decides() {
        let list = selection(&["**", "!docs/**", "docs/workflow.md"]);
        assert!(list.selects("src/lib.rs"));
        assert!(!list.selects("docs/architecture.md"));
        assert!(list.selects("docs/workflow.md"));
        assert!(!selection(&["!docs/**"]).selects("src/lib.rs"), "nothing is selected until a glob selects it");
    }

    #[test]
    fn a_bad_glob_says_which_and_why() {
        let table = [
            ("", Why::Empty),
            ("a//b", Why::Segment),
            ("docs/", Why::Segment),
            ("../x", Why::Parent),
            ("a**b", Why::Double),
            ("a[bc]", Why::Syntax { found: '[' }),
            ("{a,b}", Why::Syntax { found: '{' }),
        ];
        for (glob, why) in table {
            assert_eq!(Pattern::parse(glob).unwrap_err().why, why, "{glob:?}");
        }
        let (at, fault) = Selection::parse(&["a".to_owned(), "b\\c".to_owned()]).unwrap_err();
        assert_eq!((at, fault.pattern.as_str()), (1, "b\\c"));
        assert!(fault.to_string().contains("`b\\c`"), "{fault}");
    }
}
