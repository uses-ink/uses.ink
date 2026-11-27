//! What a format plugin gives back for one file: the contract between a format (markdown,
//! obsidian) and the engine. A format reads one file's bytes; it resolves nothing. Links to other
//! files stay `LinkTarget::Unresolved` with the text as written, and the engine resolves them
//! once it knows every page.

use serde::Serialize;

use crate::diagnostic::{Diagnostic, Notice};
use crate::document::Document;
use crate::language::Language;
use crate::route::Slug;
use crate::site::{PageKind, Tag};
use crate::span::Span;
use crate::time::When;

/// A value and where in the file it was written: the evidence a decision cites.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Spanned<T> {
    /// The value.
    pub value: T,
    /// Where it is written.
    pub span: Span,
}

impl<T> Spanned<T> {
    /// `value`, written at `span`.
    pub const fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
}

/// The frontmatter keys the engine reads, each with its span. Keys the engine does not read are
/// the author's own (other tools write them) and are neither kept nor refused.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Front {
    /// `title:`.
    pub title: Option<Spanned<String>>,
    /// `date:`; `Some(None)` when it says `false`, which decides the page undated.
    pub date: Option<Spanned<Option<When>>>,
    /// `updated:`; `Some(None)` when it says `false`, which decides the page never updated.
    pub updated: Option<Spanned<Option<When>>>,
    /// `slug:`.
    pub slug: Option<Spanned<Slug>>,
    /// `tags:`.
    pub tags: Option<Spanned<Vec<Tag>>>,
    /// `publish: true` or `false`.
    pub publish: Option<Spanned<bool>>,
    /// `draft: true` means not published.
    pub draft: Option<Spanned<bool>>,
    /// `type:` (`post`, `page`).
    pub kind: Option<Spanned<PageKind>>,
    /// `description:` or `summary:`; `Some(None)` when it says `false`, which decides the page
    /// has no summary.
    pub description: Option<Spanned<Option<String>>>,
    /// `lang:`.
    pub language: Option<Spanned<Language>>,
    /// `aliases:`: other names a link may use for the page (`[[Alias]]`).
    pub aliases: Option<Spanned<Vec<String>>>,
    /// Where the frontmatter block is, if the file has one.
    pub block: Option<Span>,
}

/// One file, read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Read {
    /// The body.
    pub document: Document,
    /// The frontmatter keys the engine reads.
    pub front: Front,
    /// The text of the first level-1 heading and where it is: a title when frontmatter has none.
    pub heading: Option<Spanned<String>>,
    /// The tags written in the text (Obsidian's `#tag`), in the order written, each where it is.
    /// The frontmatter's are in [`Front::tags`]; the engine joins the two.
    pub inline_tags: Vec<Spanned<Tag>>,
    /// Every problem the format found, in the order found.
    pub diagnostics: Vec<Diagnostic>,
    /// Everything else the format has to say.
    pub notices: Vec<Notice>,
}

impl Read {
    /// A file read without a problem or a notice.
    pub fn new(document: Document, front: Front, heading: Option<Spanned<String>>) -> Self {
        Self { document, front, heading, inline_tags: Vec::new(), diagnostics: Vec::new(), notices: Vec::new() }
    }
}
