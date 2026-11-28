//! A page, its kind, its identity and who wrote it.

use crate::{ContentPath, Document, Hash, Route, Tag, When};
use serde::{Deserialize, Serialize};
use std::fmt;

/// What a page is to its site.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum PageKind {
    /// The site's home page.
    Home,
    /// A dated page in a collection.
    Post,
    /// A page that is not a post.
    Page,
}

/// A page's identity: the hash of its file's path, so it survives every change but a rename.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PageId(Hash);

impl PageId {
    /// Hashed with a prefix, so a page id is never the hash of some blob.
    pub fn of(file: &ContentPath) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"uses.ink/page\0");
        hasher.update(file.as_str().as_bytes());
        Self(hasher.finalize().into())
    }

    /// The hash it is.
    pub const fn hash(self) -> Hash {
        self.0
    }
}

impl From<Hash> for PageId {
    /// The page whose id is `hash`, as stored or sent across the plugin boundary; whether a site
    /// holds it is the site's to say.
    fn from(hash: Hash) -> Self {
        Self(hash)
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)
    }
}

/// One routed document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Page {
    /// Its identity, from its file's path.
    pub id: PageId,
    /// The file it is read from.
    pub file: ContentPath,
    /// Where it is served.
    pub route: Route,
    /// What it is to its site.
    pub kind: PageKind,
    /// Its title.
    pub title: String,
    /// When it was written, or `None`.
    pub date: Option<When>,
    /// When it was last changed, or `None`.
    pub updated: Option<When>,
    /// Its tags, sorted by slug, each slug once.
    pub tags: Vec<Tag>,
    /// For feeds, listings and `og:description`; `None` when the page has no text.
    pub summary: Option<String>,
    /// Its body.
    pub body: Document,
}

impl Page {
    /// A page with an empty body, no date, no update, no tags and no summary; its id is derived
    /// from `file`. The rest is added by [`Page::holding`], [`Page::dated`], [`Page::tagged`]
    /// and [`Page::summarized`].
    pub fn new(file: ContentPath, route: Route, kind: PageKind, title: impl Into<String>) -> Self {
        let id = PageId::of(&file);
        let title = title.into();
        let body = Document::default();
        Self { id, file, route, kind, title, date: None, updated: None, tags: Vec::new(), summary: None, body }
    }

    /// The same page with `body`.
    #[must_use]
    pub fn holding(mut self, body: Document) -> Self {
        self.body = body;
        self
    }

    /// The same page with `summary`.
    #[must_use]
    pub fn summarized(mut self, summary: Option<String>) -> Self {
        self.summary = summary;
        self
    }

    /// The same page, written at `date` and last changed at `updated`.
    #[must_use]
    pub fn dated(mut self, date: Option<When>, updated: Option<When>) -> Self {
        self.date = date;
        self.updated = updated;
        self
    }

    /// The same page with these tags, sorted by slug, each slug once.
    #[must_use]
    pub fn tagged(mut self, mut tags: Vec<Tag>) -> Self {
        tags.sort();
        tags.dedup_by(|a, b| a.slug == b.slug);
        self.tags = tags;
        self
    }
}

/// Someone who wrote the site, by the name they go by.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Author(String);

/// An author's name was empty or only spaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyAuthor;

impl std::fmt::Display for EmptyAuthor {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("an author's name has at least one character that is not a space")
    }
}

impl std::error::Error for EmptyAuthor {}

impl TryFrom<String> for Author {
    type Error = EmptyAuthor;

    /// The name, trimmed.
    fn try_from(name: String) -> Result<Self, Self::Error> {
        let trimmed = name.trim();
        if trimmed.is_empty() { Err(EmptyAuthor) } else { Ok(Self(trimmed.to_owned())) }
    }
}

impl From<Author> for String {
    fn from(author: Author) -> Self {
        author.0
    }
}

impl AsRef<str> for Author {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Author {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(&self.0)
    }
}
