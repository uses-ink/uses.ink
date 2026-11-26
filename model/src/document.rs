//! A page's body as a typed tree. Links point at pages and anchors, never at strings, except
//! links out of the site and links that resolved to nothing.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{ContentPath, ExternalUrl, PageId, Slug};

/// A page's body.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
pub struct Document {
    pub blocks: Vec<Block>,
    /// The footnotes, in the order they are first referenced: a footnote's number is its place
    /// here, counted from 1.
    pub footnotes: Vec<Footnote>,
}

impl Document {
    /// A document without footnotes.
    pub fn new(blocks: Vec<Block>) -> Self {
        Self { blocks, footnotes: Vec::new() }
    }

    /// The same document with these footnotes, in order of first reference.
    #[must_use]
    pub fn with_footnotes(self, footnotes: Vec<Footnote>) -> Self {
        Self { footnotes, ..self }
    }

    /// The number a reference to `name` shows, from 1, or `None` when no footnote has that name.
    pub fn footnote_number(&self, name: &FootnoteName) -> Option<usize> {
        self.footnotes.iter().position(|f| &f.name == name).map(|at| at + 1)
    }
}

/// A footnote's text, referenced from the body by `Inline::Footnote`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
pub struct Footnote {
    pub name: FootnoteName,
    pub blocks: Vec<Block>,
}

impl Footnote {
    pub fn new(name: FootnoteName, blocks: Vec<Block>) -> Self {
        Self { name, blocks }
    }
}

/// A footnote's name as a slug (`[^Some note]` is `some-note`): the anchor a theme gives it is
/// built from this.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FootnoteName(Slug);

impl From<Slug> for FootnoteName {
    fn from(slug: Slug) -> Self {
        Self(slug)
    }
}

impl AsRef<str> for FootnoteName {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for FootnoteName {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, out)
    }
}

/// One item of a list.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
pub struct Item {
    /// `Some` for a task list item (`- [ ]`, `- [x]`).
    pub task: Option<Task>,
    pub blocks: Vec<Block>,
}

impl Item {
    /// A plain item.
    pub fn new(blocks: Vec<Block>) -> Self {
        Self { task: None, blocks }
    }

    /// A task list item.
    pub fn task(task: Task, blocks: Vec<Block>) -> Self {
        Self { task: Some(task), blocks }
    }
}

/// Whether a task list item is done.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Task {
    Open,
    Done,
}

/// One block of a document.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Block {
    Heading {
        level: Level,
        anchor: Anchor,
        inlines: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    /// `ordered` is the first number of an ordered list. A tight list has no blank lines
    /// between its items.
    List {
        ordered: Option<u64>,
        items: Vec<Item>,
        tight: bool,
    },
    Quote(Vec<Block>),
    Code {
        language: Option<String>,
        text: String,
    },
    Rule,
    /// `head` and each row hold one cell per alignment.
    Table {
        alignments: Vec<Alignment>,
        head: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Callout {
        kind: CalloutKind,
        title: Option<Vec<Inline>>,
        blocks: Vec<Block>,
    },
    /// Display math, as TeX.
    Math(String),
}

/// One run of text inside a block.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Inline {
    Text(String),
    Emphasis(Vec<Inline>),
    Strong(Vec<Inline>),
    Strike(Vec<Inline>),
    Code(String),
    Link {
        target: LinkTarget,
        children: Vec<Inline>,
    },
    Image {
        source: ImageSource,
        alt: String,
    },
    /// A hard line break.
    Break,
    /// A line break in the source that renders as a space.
    SoftBreak,
    /// Inline math, as TeX.
    Math(String),
    /// A reference to a footnote of the same document.
    Footnote(FootnoteName),
}

/// Where a link goes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum LinkTarget {
    Page {
        id: PageId,
        anchor: Option<Anchor>,
    },
    /// A place on the same page.
    Anchor(Anchor),
    External(ExternalUrl),
    /// A link that resolved to nothing, rendered as plain text (`link.target.missing`).
    Unresolved {
        written: String,
    },
}

/// Where an image's bytes come from.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ImageSource {
    /// A file in the snapshot, published as an asset.
    Asset(ContentPath),
    External(ExternalUrl),
    /// An image that resolved to nothing, rendered as its alt text.
    Unresolved {
        written: String,
    },
}

/// A table column's alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Alignment {
    None,
    Left,
    Center,
    Right,
}

/// What a callout says about its content (`> [!warning]`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum CalloutKind {
    Note,
    Tip,
    Important,
    Warning,
    Caution,
    /// Any other kind a format knows, such as Obsidian's `todo`.
    Other(Slug),
}

/// A heading level, 1 to 6.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "u8", try_from = "u8")]
pub struct Level(u8);

/// A number that is no heading level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelError {
    pub found: u8,
}

impl fmt::Display for LevelError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "a heading level is 1 to 6, not {}", self.found)
    }
}

impl std::error::Error for LevelError {}

impl Level {
    pub const fn new(level: u8) -> Result<Self, LevelError> {
        if 1 <= level && level <= 6 { Ok(Self(level)) } else { Err(LevelError { found: level }) }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for Level {
    type Error = LevelError;

    fn try_from(level: u8) -> Result<Self, Self::Error> {
        Self::new(level)
    }
}

impl From<Level> for u8 {
    fn from(level: Level) -> Self {
        level.0
    }
}

/// A place in a page a link can point at: a heading's slug or a block id, without the `#`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Anchor(Slug);

impl Anchor {
    pub fn new(slug: Slug) -> Self {
        Self(slug)
    }

    /// The anchor of a heading's text.
    pub fn of_heading(text: impl AsRef<str>) -> Result<Self, crate::SlugError> {
        Slug::from_text(text).map(Self)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for Anchor {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)
    }
}
