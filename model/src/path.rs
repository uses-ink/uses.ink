//! Paths of files in a snapshot.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A file's path in a snapshot: relative, segments joined by `/`, never empty.
///
/// No leading `/`, no empty segment (so no trailing or doubled `/`), no `.` or `..` segment, no
/// backslash, no NUL. Ordered by its bytes, so `a-b` sorts before `a/b`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct ContentPath(String);

/// Why text is not a [`ContentPath`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathError {
    Empty,
    /// It starts with `/`.
    Absolute,
    /// Segment number `index` (from zero) is empty.
    EmptySegment {
        index: usize,
    },
    /// Segment number `index` is `.` or `..`.
    Relative {
        index: usize,
    },
    /// A backslash at this byte offset.
    Backslash {
        at: usize,
    },
    /// A NUL at this byte offset.
    Nul {
        at: usize,
    },
}

impl fmt::Display for PathError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(out, "a path is not empty"),
            Self::Absolute => write!(out, "a path in a snapshot does not start with `/`"),
            Self::EmptySegment { index } => write!(out, "part {} of the path is empty", index + 1),
            Self::Relative { index } => write!(out, "part {} of the path is `.` or `..`", index + 1),
            Self::Backslash { at } => write!(out, "a backslash at byte {at}; paths use `/`"),
            Self::Nul { at } => write!(out, "a NUL byte at byte {at}"),
        }
    }
}

impl std::error::Error for PathError {}

impl ContentPath {
    pub fn parse(text: impl Into<String>) -> Result<Self, PathError> {
        let text = text.into();
        if text.is_empty() {
            return Err(PathError::Empty);
        }
        if text.starts_with('/') {
            return Err(PathError::Absolute);
        }
        if let Some(at) = text.find('\\') {
            return Err(PathError::Backslash { at });
        }
        if let Some(at) = text.find('\0') {
            return Err(PathError::Nul { at });
        }
        for (index, segment) in text.split('/').enumerate() {
            match segment {
                "" => return Err(PathError::EmptySegment { index }),
                "." | ".." => return Err(PathError::Relative { index }),
                _ => {}
            }
        }
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Its segments, first to last; never empty.
    pub fn segments(&self) -> impl DoubleEndedIterator<Item = &str> {
        self.0.split('/')
    }

    /// The last segment: `posts/hello.md` is `hello.md`.
    pub fn file_name(&self) -> &str {
        self.0.rsplit_once('/').map_or(self.0.as_str(), |(_, name)| name)
    }

    /// The file name before its last `.`: `hello.md` is `hello`, `a.tar.gz` is `a.tar`. A name
    /// whose only `.` is its first character (`.obsidian`) is its own stem.
    pub fn stem(&self) -> &str {
        let name = self.file_name();
        match name.rsplit_once('.') {
            Some((stem, _)) if !stem.is_empty() => stem,
            _ => name,
        }
    }

    /// The file name after its last `.`, if it has one past the first character.
    pub fn extension(&self) -> Option<&str> {
        let name = self.file_name();
        match name.rsplit_once('.') {
            Some((stem, extension)) if !stem.is_empty() => Some(extension),
            _ => None,
        }
    }

    /// The folder holding it, or `None` at the top of the snapshot.
    pub fn parent(&self) -> Option<Self> {
        self.0.rsplit_once('/').map(|(parent, _)| Self(parent.to_owned()))
    }

    /// `other` under this path: `posts` joined with `a/b.md` is `posts/a/b.md`.
    #[must_use]
    pub fn join(&self, other: &Self) -> Self {
        Self(format!("{}/{}", self.0, other.0))
    }

    /// Whether `self` is `folder` or under it.
    pub fn is_within(&self, folder: &Self) -> bool {
        self.0.strip_prefix(folder.as_str()).is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    }

    /// This path relative to `folder`, or `None` when it is not strictly under it.
    pub fn strip(&self, folder: &Self) -> Option<Self> {
        let rest = self.0.strip_prefix(folder.as_str())?.strip_prefix('/')?;
        Some(Self(rest.to_owned()))
    }
}

impl fmt::Display for ContentPath {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

impl FromStr for ContentPath {
    type Err = PathError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl TryFrom<String> for ContentPath {
    type Error = PathError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(text)
    }
}

impl TryFrom<&str> for ContentPath {
    type Error = PathError;

    fn try_from(text: &str) -> Result<Self, Self::Error> {
        Self::parse(text)
    }
}

impl From<ContentPath> for String {
    fn from(path: ContentPath) -> Self {
        path.0
    }
}

impl AsRef<str> for ContentPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use proptest::prelude::*;

    use super::{ContentPath, PathError};

    fn path(text: &str) -> ContentPath {
        ContentPath::parse(text).unwrap()
    }

    #[test]
    fn refuses_each_bad_shape() {
        let refused = |text: &str| ContentPath::parse(text).unwrap_err();
        assert_eq!(refused(""), PathError::Empty);
        assert_eq!(refused("/a"), PathError::Absolute);
        assert_eq!(refused("a//b"), PathError::EmptySegment { index: 1 });
        assert_eq!(refused("a/"), PathError::EmptySegment { index: 1 });
        assert_eq!(refused("a/../b"), PathError::Relative { index: 1 });
        assert_eq!(refused("./a"), PathError::Relative { index: 0 });
        assert_eq!(refused("a\\b"), PathError::Backslash { at: 1 });
        assert_eq!(refused("a\0"), PathError::Nul { at: 1 });
    }

    #[test]
    fn names_its_parts() {
        let p = path("posts/2024/a.tar.gz");
        assert_eq!(p.segments().collect::<Vec<_>>(), ["posts", "2024", "a.tar.gz"]);
        assert_eq!(p.file_name(), "a.tar.gz");
        assert_eq!(p.stem(), "a.tar");
        assert_eq!(p.extension(), Some("gz"));
        assert_eq!(p.parent(), Some(path("posts/2024")));
        assert_eq!(path("README").parent(), None);
        assert_eq!(path(".obsidian").stem(), ".obsidian");
        assert_eq!(path(".obsidian").extension(), None);
        assert_eq!(path("posts").join(&path("a/b.md")), path("posts/a/b.md"));
        assert!(path("posts/a.md").is_within(&path("posts")));
        assert!(!path("postscript.md").is_within(&path("posts")));
        assert_eq!(path("posts/a.md").strip(&path("posts")), Some(path("a.md")));
    }

    proptest! {
        #[test]
        fn joined_segments_parse_back(segments in prop::collection::vec("[^/\\\\\0]{1,8}", 1..5)) {
            prop_assume!(segments.iter().all(|s| s != "." && s != ".."));
            let text = segments.join("/");
            let parsed = ContentPath::parse(text.clone()).unwrap();
            prop_assert_eq!(parsed.as_str(), text.as_str());
            prop_assert_eq!(parsed.segments().count(), segments.len());
        }

        #[test]
        fn parsing_never_panics(text in "\\PC{0,24}") {
            let _parsed = ContentPath::parse(text);
        }
    }
}
