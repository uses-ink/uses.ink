//! What a dialect of markdown adds to the `CommonMark` this crate reads: which syntax comrak
//! parses ([`Syntax`]) and what the dialect's own nodes mean ([`Dialect`]). comrak stays inside
//! this crate; a dialect sees model types and the text as written. [`Markdown`] is the plain
//! dialect, the one [`crate::read`] reads.

use model::{Announcement, CalloutKind, Inline, Read, Span};

pub use crate::body::Plain;

/// The syntax a dialect reads beyond `CommonMark`, tables, strikethrough, autolinks, footnotes,
/// task lists, highlight and math, which every dialect reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Syntax {
    /// How `> [!kind]` is read.
    pub callouts: Callouts,
    /// Where a note's title comes from.
    pub title: Titled,
    /// The extensions turned on.
    pub extensions: &'static [Extension],
}

impl Syntax {
    /// Whether `extension` is on.
    pub fn has(self, extension: Extension) -> bool {
        self.extensions.contains(&extension)
    }
}

/// Syntax a dialect may turn on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Extension {
    /// `^x^` as superscript and `~x~` as subscript.
    Scripts,
    /// `Term` then `: details` as a definition list.
    Definitions,
    /// `[[target]]` and `![[target]]`, handed to [`Dialect::wikilink`].
    Wikilinks,
    /// `^id` ending a block, and alone on the line after one, as an anchor.
    BlockIds,
    /// `%%text%%` and `%%` blocks as comments, removed like HTML comments.
    Comments,
    /// The frontmatter keys `aliases` and `alias`, into [`model::Front::aliases`].
    Aliases,
    /// Strikethrough is `~~x~~` only: a single-tilde pair is text, as written.
    DoubleStrike,
}

/// Where a note's title comes from, when its frontmatter has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Titled {
    /// The first level-1 heading, then the file name.
    Heading,
    /// The file name: a heading is a heading, never the title, as in Obsidian.
    File,
}

/// How `> [!kind]` is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Callouts {
    /// GitHub's five alerts, each with an optional title on the same line.
    Alerts,
    /// Obsidian's callouts: any kind, through [`Dialect::callout`], a title of inlines, and `+`
    /// or `-` for a callout a reader can fold.
    Folding,
}

/// A wikilink as written: `[[target|label]]`, or `![[target]]` for an embed.
#[derive(Debug)]
#[non_exhaustive]
pub struct Wikilink<'a> {
    /// What is between the brackets, before any `|`.
    pub target: &'a str,
    /// The text after the `|`, or the target when there is none, read as inlines.
    pub label: Vec<Inline>,
    /// Whether a `!` comes before it.
    pub embedded: bool,
    /// Where it is in the file.
    pub span: Span,
}

/// What a wikilink becomes.
#[derive(Debug)]
pub struct Linked {
    /// The inline in its place: a link, an image, or text.
    pub inline: Inline,
    /// For an embed of a page, its target as written: alone in its paragraph it becomes a
    /// [`model::Block::Embed`]; among other inlines it becomes an [`Inline::Embed`], which the
    /// engine turns into a block between two paragraphs.
    pub embed: Option<String>,
    /// What is worth saying about it, such as a part of it that was not kept.
    pub said: Option<Announcement>,
}

impl From<Inline> for Linked {
    fn from(inline: Inline) -> Self {
        Self { inline, embed: None, said: None }
    }
}

/// A dialect of markdown: its syntax and what its own nodes mean. One value reads one file, so
/// it may gather what it sees (tags in the text) and add it to the read at the end.
pub trait Dialect {
    /// The syntax comrak parses.
    const SYNTAX: Syntax;

    /// What a wikilink becomes. Called only when [`Extension::Wikilinks`] is on.
    fn wikilink(&mut self, link: Wikilink<'_>) -> Linked;

    /// The kind a callout written `[!kind]` is, `kind` in lower case, or `None` for a kind the
    /// dialect does not know. Called only for [`Callouts::Folding`].
    fn callout(&self, kind: &str) -> Option<CalloutKind>;

    /// Text of the body, as written, and where it is: a dialect reading meaning out of plain
    /// text (`#tag`) sees every run of it, outside code.
    fn text(&mut self, text: &str, span: Span);

    /// The read, finished: what the dialect gathered is added to it.
    fn finish(self, read: &mut Read);
}

/// `CommonMark` and GitHub's extensions, the format this crate is: what [`crate::read`] reads.
#[derive(Clone, Copy, Debug, Default)]
pub struct Markdown;

impl Dialect for Markdown {
    const SYNTAX: Syntax = Syntax {
        callouts: Callouts::Alerts,
        title: Titled::Heading,
        extensions: &[Extension::Scripts, Extension::Definitions],
    };

    /// Never called: wikilinks are off, so `[[x]]` stays text.
    fn wikilink(&mut self, link: Wikilink<'_>) -> Linked {
        Inline::Text(link.target.to_owned()).into()
    }

    /// Never called: alerts have their own five kinds.
    fn callout(&self, _: &str) -> Option<CalloutKind> {
        None
    }

    fn text(&mut self, _: &str, _: Span) {}

    fn finish(self, _: &mut Read) {}
}
