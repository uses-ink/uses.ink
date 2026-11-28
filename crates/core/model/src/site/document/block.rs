//! Block nodes: paragraphs, lists, tables, callouts and the rest.

use crate::{Anchor, Inline, Level, LinkTarget, Slug};
use serde::{Deserialize, Serialize};

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
