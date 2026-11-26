//! Glob patterns over file paths, for `include` and `exclude`.
//!
//! A pattern is a path whose parts may hold wildcards:
//!
//! - `*` matches any run of characters within one part, the empty run included;
//! - `?` matches one character within one part;
//! - `**`, alone as a part, matches any number of whole parts, none included.
//!
//! Nothing else is special. A pattern that matches a folder matches everything in it, so
//! `drafts` and `drafts/**` both match `drafts/a.md`, and `*.md` matches `notes.md` but not
//! `notes/a.md`. Matching is by characters and case-sensitive.

use std::fmt;
use std::str::FromStr;

use model::ContentPath;

/// One `include` or `exclude` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    text: String,
    parts: Vec<Part>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    /// `**`.
    Any,
    Glob(Vec<Piece>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Piece {
    Char(char),
    /// `*`.
    Run,
    /// `?`.
    One,
}

/// Why text is not a [`Pattern`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PatternError {
    /// Nothing was written.
    Empty,
    /// It starts with `/`; patterns are relative to the source root.
    Absolute,
    /// Part number `index` (from zero) is empty, as in `a//b` or `a/`.
    EmptyPart {
        /// Which part, from zero.
        index: usize,
    },
    /// Part number `index` is `.` or `..`.
    Relative {
        /// Which part, from zero.
        index: usize,
    },
    /// Part number `index` holds `**` next to other characters, as in `a**`.
    Doublestar {
        /// Which part, from zero.
        index: usize,
    },
    /// A backslash at this byte offset; paths use `/`.
    Backslash {
        /// Its byte offset.
        at: usize,
    },
}

impl fmt::Display for PatternError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(out, "a pattern is not empty"),
            Self::Absolute => write!(out, "a pattern does not start with `/`"),
            Self::EmptyPart { index } => write!(out, "part {} of the pattern is empty", index + 1),
            Self::Relative { index } => write!(out, "part {} of the pattern is `.` or `..`", index + 1),
            Self::Doublestar { index } => {
                write!(out, "part {} of the pattern has `**` next to other characters", index + 1)
            }
            Self::Backslash { at } => write!(out, "a backslash at byte {at}; paths use `/`"),
        }
    }
}

impl std::error::Error for PatternError {}

impl FromStr for Pattern {
    type Err = PatternError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() {
            return Err(PatternError::Empty);
        }
        if text.starts_with('/') {
            return Err(PatternError::Absolute);
        }
        if let Some(at) = text.find('\\') {
            return Err(PatternError::Backslash { at });
        }
        let parts =
            text.split('/').enumerate().map(|(index, part)| Part::parse(index, part)).collect::<Result<_, _>>()?;
        Ok(Self { text: text.to_owned(), parts })
    }
}

impl AsRef<str> for Pattern {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

impl Pattern {
    /// Whether it matches `path` or a folder holding it.
    pub fn matches(&self, path: &ContentPath) -> bool {
        let parts: Vec<&str> = path.segments().collect();
        (1..=parts.len()).any(|len| parts.get(..len).is_some_and(|prefix| self.matches_whole(prefix)))
    }

    /// Whether it matches exactly these parts: the wildcard walk with `**` as the run over
    /// parts and each other part matching one part.
    fn matches_whole(&self, parts: &[&str]) -> bool {
        walk(
            &self.parts,
            parts,
            |part| *part == Part::Any,
            |pattern, part| match pattern {
                Part::Any => true,
                Part::Glob(pieces) => glob(pieces, part),
            },
        )
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.text)
    }
}

impl Part {
    fn parse(index: usize, text: &str) -> Result<Self, PatternError> {
        match text {
            "" => Err(PatternError::EmptyPart { index }),
            "." | ".." => Err(PatternError::Relative { index }),
            "**" => Ok(Self::Any),
            _ if text.contains("**") => Err(PatternError::Doublestar { index }),
            _ => Ok(Self::Glob(
                text.chars()
                    .map(|c| match c {
                        '*' => Piece::Run,
                        '?' => Piece::One,
                        c => Piece::Char(c),
                    })
                    .collect(),
            )),
        }
    }
}

/// Whether one part's pieces match its characters.
fn glob(pieces: &[Piece], part: &str) -> bool {
    let chars: Vec<char> = part.chars().collect();
    walk(
        pieces,
        &chars,
        |piece| *piece == Piece::Run,
        |piece, c| match piece {
            Piece::Run | Piece::One => true,
            Piece::Char(want) => want == c,
        },
    )
}

/// The linear wildcard walk. A run matches any number of input items; any other pattern item
/// matches one input item when `one` says so. On a miss the walk goes back to the last run and
/// lets it take one more item.
fn walk<P, I>(pattern: &[P], input: &[I], is_run: impl Fn(&P) -> bool, one: impl Fn(&P, &I) -> bool) -> bool {
    let (mut p, mut i) = (0, 0);
    // the last run seen, and the input index after the items it has taken so far
    let mut back: Option<(usize, usize)> = None;
    while let Some(at) = input.get(i) {
        match pattern.get(p) {
            Some(item) if is_run(item) => {
                back = Some((p, i));
                p += 1;
            }
            Some(item) if one(item, at) => {
                p += 1;
                i += 1;
            }
            _ => match back {
                Some((run, taken)) => {
                    back = Some((run, taken + 1));
                    (p, i) = (run + 1, taken + 1);
                }
                None => return false,
            },
        }
    }
    pattern.get(p..).is_some_and(|rest| rest.iter().all(is_run))
}

#[cfg(test)]
mod test {
    use model::ContentPath;
    use proptest::prelude::*;

    use super::{Pattern, PatternError};

    fn pattern(text: &str) -> Pattern {
        text.parse().unwrap()
    }

    fn matches(text: &str, path: &str) -> bool {
        pattern(text).matches(&ContentPath::parse(path).unwrap())
    }

    #[test]
    fn matches_each_wildcard() {
        assert!(matches("*.md", "a.md"));
        assert!(matches("*.md", ".md"));
        assert!(!matches("*.md", "notes/a.md"));
        assert!(matches("notes/*.md", "notes/a.md"));
        assert!(matches("a?c", "abc"));
        assert!(!matches("a?c", "ac"));
        assert!(matches("**/*.md", "a.md"));
        assert!(matches("**/*.md", "x/y/a.md"));
        assert!(matches("notes/**/2023.md", "notes/2023.md"));
        assert!(matches("notes/**/2023.md", "notes/a/b/2023.md"));
        assert!(!matches("notes/**/2023.md", "other/2023.md"));
        assert!(matches("a*b*c", "aXbYbZc"));
        assert!(!matches("a*b*c", "aXbYbZ"));
        assert!(matches("é?", "éü"));
    }

    #[test]
    fn a_folder_match_holds_its_files() {
        assert!(matches("drafts", "drafts/a.md"));
        assert!(matches("drafts/**", "drafts/a/b.md"));
        assert!(matches("notes/private/**", "notes/private/x.md"));
        assert!(!matches("drafts", "drafts-old/a.md"));
        assert!(!matches("notes/private/**", "notes/public.md"));
    }

    #[test]
    fn refuses_each_bad_shape() {
        let refused = |text: &str| text.parse::<Pattern>().unwrap_err();
        assert_eq!(refused(""), PatternError::Empty);
        assert_eq!(refused("/a"), PatternError::Absolute);
        assert_eq!(refused("a//b"), PatternError::EmptyPart { index: 1 });
        assert_eq!(refused("a/"), PatternError::EmptyPart { index: 1 });
        assert_eq!(refused("../a"), PatternError::Relative { index: 0 });
        assert_eq!(refused("a/b**"), PatternError::Doublestar { index: 1 });
        assert_eq!(refused("a\\b"), PatternError::Backslash { at: 1 });
    }

    proptest! {
        #[test]
        fn a_literal_matches_itself(segments in prop::collection::vec("[a-z.]{1,6}", 1..4)) {
            prop_assume!(segments.iter().all(|s| s != "." && s != ".."));
            let text = segments.join("/");
            prop_assert!(matches(&text, &text));
            prop_assert!(matches("**", &text));
            let starred: Vec<&str> = segments.iter().map(|_| "*").collect();
            prop_assert!(matches(&starred.join("/"), &text));
        }
    }
}
