//! What a format plugin gives back for one file: the contract between a format (markdown,
//! obsidian) and the engine. A format reads one file's bytes; it resolves nothing. Links to other
//! files stay `LinkTarget::Unresolved` with the text as written, and the engine resolves them
//! once it knows every page.

use serde::Serialize;

use crate::diagnostic::{Diagnostic, Notice};
use crate::document::Document;
use crate::route::Slug;
use crate::site::{PageKind, Tag};
use crate::span::Span;
use crate::time::When;

/// A value and where in the file it was written: the evidence a decision cites.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub const fn new(value: T, span: Span) -> Self {
        Self { value, span }
    }
}

/// The frontmatter keys the engine reads, each with its span. Keys the engine does not read are
/// the author's own (other tools write them) and are neither kept nor refused.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Front {
    pub title: Option<Spanned<String>>,
    pub date: Option<Spanned<When>>,
    pub updated: Option<Spanned<When>>,
    pub slug: Option<Spanned<Slug>>,
    pub tags: Option<Spanned<Vec<Tag>>>,
    /// `publish: true` or `false`.
    pub publish: Option<Spanned<bool>>,
    /// `draft: true` means not published.
    pub draft: Option<Spanned<bool>>,
    /// `type:` (`post`, `page`).
    pub kind: Option<Spanned<PageKind>>,
    /// Where the frontmatter block is, if the file has one.
    pub block: Option<Span>,
}

/// One file, read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Read {
    pub document: Document,
    pub front: Front,
    /// The text of the first level-1 heading and where it is: a title when frontmatter has none.
    pub heading: Option<Spanned<String>>,
    pub diagnostics: Vec<Diagnostic>,
    pub notices: Vec<Notice>,
}

impl Read {
    pub fn new(document: Document, front: Front, heading: Option<Spanned<String>>) -> Self {
        Self { document, front, heading, diagnostics: Vec::new(), notices: Vec::new() }
    }
}
