//! The interpreted site: pages with routes, assets, navigation, and what was left out and why.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{ContentPath, DecisionCode, Document, Hash, Language, Route, Slug, When};

/// Where a site's content starts in the snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Root {
    /// The top of the snapshot.
    Top,
    /// A folder of the snapshot, such as `docs`.
    Folder(ContentPath),
}

/// How a site's files are arranged, which sets the defaults of later decisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Layout {
    Blog,
    Vault,
    Docs,
    Flat,
}

/// What a page is to its site.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum PageKind {
    Home,
    /// A dated page in a collection.
    Post,
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

    pub const fn hash(self) -> Hash {
        self.0
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)
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

/// A tag as the author wrote it, with its slug for routes and comparison.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "TagParts")]
#[non_exhaustive]
pub struct Tag {
    pub slug: Slug,
    pub written: String,
}

/// A tag as stored, before its slug is checked against its text.
#[derive(Debug, Deserialize)]
struct TagParts {
    slug: Slug,
    written: String,
}

/// Why stored parts are no [`Tag`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TagError {
    /// The text has no letter or digit.
    Empty {
        /// Why it makes no slug.
        source: crate::SlugError,
    },
    /// The stored slug is not the slug of the text.
    Slug {
        /// The slug of the text.
        expected: Slug,
        /// The slug stored with it.
        found: Slug,
    },
}

impl fmt::Display for TagError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty { .. } => out.write_str("a tag needs at least one letter or digit"),
            Self::Slug { expected, found } => {
                write!(out, "the tag's slug is `{found}`, but its text gives `{expected}`")
            }
        }
    }
}

impl std::error::Error for TagError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Empty { source } => Some(source),
            Self::Slug { .. } => None,
        }
    }
}

impl TryFrom<TagParts> for Tag {
    type Error = TagError;

    fn try_from(parts: TagParts) -> Result<Self, Self::Error> {
        let tag = Self::new(parts.written).map_err(|source| TagError::Empty { source })?;
        if tag.slug == parts.slug { Ok(tag) } else { Err(TagError::Slug { expected: tag.slug, found: parts.slug }) }
    }
}

impl Tag {
    /// A tag from what the author wrote; it needs at least one letter or digit.
    pub fn new(written: impl Into<String>) -> Result<Self, crate::SlugError> {
        let written = written.into();
        Ok(Self { slug: Slug::from_text(&written)?, written })
    }
}

/// One routed document.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Page {
    pub id: PageId,
    pub file: ContentPath,
    pub route: Route,
    pub kind: PageKind,
    pub title: String,
    pub date: Option<When>,
    pub updated: Option<When>,
    pub tags: Vec<Tag>,
    /// For feeds, listings and `og:description`; `None` when the page has no text.
    pub summary: Option<String>,
    pub body: Document,
}

impl Page {
    /// A page with no date, no update and no tags; its id is derived from `file`.
    pub fn new(file: ContentPath, route: Route, kind: PageKind, title: impl Into<String>, body: Document) -> Self {
        let id = PageId::of(&file);
        let title = title.into();
        Self { id, file, route, kind, title, date: None, updated: None, tags: Vec::new(), summary: None, body }
    }

    #[must_use]
    pub fn summarized(mut self, summary: Option<String>) -> Self {
        self.summary = summary;
        self
    }

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

/// A file published as is because a published page uses it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[non_exhaustive]
pub struct Asset {
    pub file: ContentPath,
    pub route: Route,
    pub blob: Hash,
}

impl Asset {
    pub fn new(file: ContentPath, route: Route, blob: Hash) -> Self {
        Self { file, route, blob }
    }
}

/// A file left out of the site, and the decision that left it out.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[non_exhaustive]
pub struct Excluded {
    pub path: ContentPath,
    pub because: DecisionCode,
}

impl Excluded {
    pub fn new(path: ContentPath, because: DecisionCode) -> Self {
        Self { path, because }
    }
}

/// What a site says about itself: its name, its line, its authors, its language.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Identity {
    pub title: String,
    pub description: Option<String>,
    /// First author first.
    pub authors: Vec<Author>,
    pub language: Language,
}

impl Identity {
    /// A site with no description and no authors.
    pub fn new(title: impl Into<String>, language: Language) -> Self {
        Self { title: title.into(), description: None, authors: Vec::new(), language }
    }
}

/// The site the engine interpreted. Backlinks, tag pages and feeds are functions of it, not
/// fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Site {
    identity: Identity,
    root: Root,
    layout: Layout,
    home: Option<PageId>,
    pages: Vec<Page>,
    assets: Vec<Asset>,
    nav: Vec<PageId>,
    excluded: Vec<Excluded>,
}

/// Why parts do not make a [`Site`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SiteError {
    /// Two pages or assets share a route.
    Collision { route: Route, files: [ContentPath; 2] },
    /// The home or a nav entry names a page the site does not hold.
    Missing { page: PageId },
}

impl fmt::Display for SiteError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Collision { route, files: [a, b] } => write!(out, "`{a}` and `{b}` both route to `{route}`"),
            Self::Missing { page } => write!(out, "no page has id {page}"),
        }
    }
}

impl std::error::Error for SiteError {}

/// What a [`Site`] is made of.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct SiteParts {
    pub identity: Identity,
    pub root: Root,
    pub layout: Layout,
    pub home: Option<PageId>,
    pub pages: Vec<Page>,
    pub assets: Vec<Asset>,
    pub nav: Vec<PageId>,
    pub excluded: Vec<Excluded>,
}

impl SiteParts {
    /// A site with nothing in it yet.
    pub fn new(identity: Identity, root: Root, layout: Layout) -> Self {
        Self {
            identity,
            root,
            layout,
            home: None,
            pages: Vec::new(),
            assets: Vec::new(),
            nav: Vec::new(),
            excluded: Vec::new(),
        }
    }
}

impl Site {
    /// Pages and assets are sorted by route, exclusions by path. A route used twice, or a home
    /// or nav entry that is not a page, is an error.
    pub fn new(parts: SiteParts) -> Result<Self, SiteError> {
        let SiteParts { identity, root, layout, home, mut pages, mut assets, nav, mut excluded } = parts;
        pages.sort_by(|a, b| a.route.cmp(&b.route));
        assets.sort();
        excluded.sort();
        let mut routes: Vec<(&Route, &ContentPath)> =
            pages.iter().map(|p| (&p.route, &p.file)).chain(assets.iter().map(|a| (&a.route, &a.file))).collect();
        routes.sort();
        for pair in routes.windows(2) {
            if let [(route, a), (other, b)] = pair
                && route == other
            {
                return Err(SiteError::Collision { route: (*route).clone(), files: [(*a).clone(), (*b).clone()] });
            }
        }
        if let Some(page) = home.iter().chain(&nav).find(|id| !pages.iter().any(|p| p.id == **id)) {
            return Err(SiteError::Missing { page: *page });
        }
        Ok(Self { identity, root, layout, home, pages, assets, nav, excluded })
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    pub fn root(&self) -> &Root {
        &self.root
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// The home page, or `None` when the home is a generated listing.
    pub fn home(&self) -> Option<&Page> {
        self.home.and_then(|id| self.page(id))
    }

    /// Every page, sorted by route.
    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Every asset, sorted by route.
    pub fn assets(&self) -> &[Asset] {
        &self.assets
    }

    /// The pages in the navigation, in the author's order.
    pub fn nav(&self) -> &[PageId] {
        &self.nav
    }

    /// Every file left out, sorted by path.
    pub fn excluded(&self) -> &[Excluded] {
        &self.excluded
    }

    pub fn page(&self, id: PageId) -> Option<&Page> {
        self.pages.iter().find(|page| page.id == id)
    }
}
