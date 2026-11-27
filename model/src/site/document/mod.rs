//! A page's body as a typed tree. Links point at pages and anchors, never at strings, except
//! links out of the site and links that resolved to nothing.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Slug;

mod fold;
mod visit;

pub use fold::Fold;
pub use visit::Visit;

mod anchor;
mod block;
mod inline;

pub use anchor::{Anchor, Level, LevelError};
pub use block::{Alignment, Block, CalloutKind, Definition, Folding, Item, Task};
pub use inline::{ImageSource, Inline, LinkTarget};

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
