//! A page's body as a typed tree. Links point at pages and anchors, never at strings, except
//! links out of the site and links that resolved to nothing.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{ContentPath, ExternalUrl, PageId, Slug};

mod fold;
mod visit;

pub use fold::Fold;
pub use visit::Visit;

/// A page's body.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
pub struct Document {
    /// The body, in reading order.
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
    /// The name references use.
    pub name: FootnoteName,
    /// Its text.
    pub blocks: Vec<Block>,
}

impl Footnote {
    /// The footnote `name` with `blocks` as its text.
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
    /// Its content.
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
    /// `- [ ]`: not done yet.
    Open,
    /// `- [x]`: done.
    Done,
}

/// One block of a document.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Block {
    /// A heading, `#` to `######`.
    Heading {
        /// 1 to 6.
        level: Level,
        /// The place links to it point at.
        anchor: Anchor,
        /// Its text.
        inlines: Vec<Inline>,
    },
    /// A paragraph of text.
    Paragraph(Vec<Inline>),
    /// `ordered` is the first number of an ordered list. A tight list has no blank lines
    /// between its items.
    List {
        /// The first number of an ordered list; `None` for a bulleted one.
        ordered: Option<u64>,
        /// Its items, in order.
        items: Vec<Item>,
        /// Whether no blank line separates its items.
        tight: bool,
    },
    /// A block quote.
    Quote(Vec<Block>),
    /// A fenced or indented code block.
    Code {
        /// The language written after the opening fence, if any.
        language: Option<String>,
        /// The code, as written.
        text: String,
    },
    /// A thematic break: `---`.
    Rule,
    /// `head` and each row hold one cell per alignment.
    Table {
        /// Each column's alignment, left to right.
        alignments: Vec<Alignment>,
        /// The header row's cells.
        head: Vec<Vec<Inline>>,
        /// Every body row's cells.
        rows: Vec<Vec<Vec<Inline>>>,
    },
    /// A callout: a quote that says what kind of note it is (`> [!tip]`).
    Callout {
        /// What it says about its content.
        kind: CalloutKind,
        /// Its own title, or `None` for its kind's name.
        title: Option<Vec<Inline>>,
        /// Its content.
        blocks: Vec<Block>,
        /// Whether a reader can fold it, and how it starts.
        folding: Folding,
    },
    /// Display math, as TeX.
    Math(String),
    /// Terms and their details (`Term` then `: details`).
    Definitions(Vec<Definition>),
    /// A section shown folded, with the summary as its handle (`<details><summary>`).
    Details {
        /// What shows while it is folded.
        summary: Vec<Inline>,
        /// What shows once it is open.
        blocks: Vec<Block>,
    },
    /// Another page's content shown in place (`![[Note]]`, `![[Note#Heading]]`). A format gives
    /// the target as written and no blocks; the engine resolves the target and puts the page's
    /// blocks, or the part the anchor names, in `blocks`.
    Embed {
        /// The page embedded, and the part of it when an anchor names one.
        target: LinkTarget,
        /// What is shown: the page's blocks, or the part the anchor names.
        blocks: Vec<Block>,
    },
}

/// Whether a reader can fold a callout, and how it starts (`> [!note]-`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Folding {
    /// Always open: no `+` or `-` after the kind.
    #[default]
    Fixed,
    /// Foldable, open at first: `+`.
    Open,
    /// Foldable, folded at first: `-`.
    Closed,
}

/// One term of a definition list and its details, each a run of blocks.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
pub struct Definition {
    /// The term defined.
    pub term: Vec<Inline>,
    /// Each of its details, in order.
    pub details: Vec<Vec<Block>>,
}

impl Definition {
    /// `term`, with `details`.
    pub fn new(term: Vec<Inline>, details: Vec<Vec<Block>>) -> Self {
        Self { term, details }
    }
}

/// One run of text inside a block.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Inline {
    /// Plain text.
    Text(String),
    /// `*text*`.
    Emphasis(Vec<Inline>),
    /// `**text**`.
    Strong(Vec<Inline>),
    /// `~~text~~`.
    Strike(Vec<Inline>),
    /// `` `code` ``, as written.
    Code(String),
    /// A link.
    Link {
        /// Where it goes.
        target: LinkTarget,
        /// Its text.
        children: Vec<Inline>,
    },
    /// An image.
    Image {
        /// Where its bytes come from.
        source: ImageSource,
        /// The text shown in its place, and read by screen readers.
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
    /// Marked text (`==text==`, `<mark>`).
    Highlight(Vec<Inline>),
    /// `^text^`, `<sup>`.
    Superscript(Vec<Inline>),
    /// `~text~`, `<sub>`.
    Subscript(Vec<Inline>),
    /// A key or key combination (`<kbd>Ctrl</kbd>`).
    Keyboard(String),
    /// A place a link can point at, inside a block: a block id (`^intro`). Shows nothing.
    Anchor(Anchor),
    /// Another page's content shown in place, written among other text (`see ![[Note]] here`). A
    /// format gives the target as written; the engine ends the paragraph before it and starts one
    /// after it, with a [`Block::Embed`] between, since blocks cannot sit inside text. Where the
    /// engine cannot (a heading, a table cell, a link's text) it becomes a link, so a finished
    /// page holds none.
    Embed {
        /// The page embedded, and the part of it when an anchor names one.
        target: LinkTarget,
    },
}

/// Where a link goes.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum LinkTarget {
    /// A page of the site.
    Page {
        /// The page.
        id: PageId,
        /// A place on it, or `None` for its top.
        anchor: Option<Anchor>,
    },
    /// A place on the same page.
    Anchor(Anchor),
    /// A place outside the site.
    External(ExternalUrl),
    /// A link that resolved to nothing, rendered as plain text (`link.target.missing`).
    Unresolved {
        /// The target, as written.
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
    /// An image outside the site.
    External(ExternalUrl),
    /// An image that resolved to nothing, rendered as its alt text.
    Unresolved {
        /// The source, as written.
        written: String,
    },
}

/// A table column's alignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Alignment {
    /// As the theme aligns text.
    None,
    /// `:---`.
    Left,
    /// `:---:`.
    Center,
    /// `---:`.
    Right,
}

/// What a callout says about its content (`> [!warning]`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum CalloutKind {
    /// `> [!note]`: worth knowing.
    Note,
    /// `> [!tip]`: a way to do better.
    Tip,
    /// `> [!important]`: needed to succeed.
    Important,
    /// `> [!warning]`: needs attention now.
    Warning,
    /// `> [!caution]`: a risk of harm.
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
    /// The number given.
    pub found: u8,
}

impl fmt::Display for LevelError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "a heading level is {} to {}, not {}", Level::MIN, Level::MAX, self.found)
    }
}

impl std::error::Error for LevelError {}

impl Level {
    /// The lowest heading level, `<h1>`.
    const MIN: u8 = 1;
    /// The highest heading level, `<h6>`.
    const MAX: u8 = 6;

    /// `level`, when it is 1 to 6.
    pub const fn new(level: u8) -> Result<Self, LevelError> {
        if Self::MIN <= level && level <= Self::MAX { Ok(Self(level)) } else { Err(LevelError { found: level }) }
    }

    /// The level, 1 to 6.
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
    /// The anchor `slug`.
    pub fn new(slug: Slug) -> Self {
        Self(slug)
    }

    /// The anchor of a heading's text.
    pub fn of_heading(text: impl AsRef<str>) -> Result<Self, crate::SlugError> {
        Slug::from_text(text).map(Self)
    }

    /// The anchor's text, without the `#`.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<Slug> for Anchor {
    fn from(slug: Slug) -> Self {
        Self(slug)
    }
}

impl fmt::Display for Anchor {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)
    }
}
