//! Inline nodes and what a link or an image points at.

use crate::{Anchor, ContentPath, ExternalUrl, FootnoteName, PageId};
use serde::Serialize;

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
