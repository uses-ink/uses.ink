//! The interpreted site: pages with routes, assets, navigation, and what was left out and why.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{ContentPath, DecisionCode, Document, Hash, Root, Route, Slug, When};

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

/// A tag as the author wrote it, with its slug for routes and comparison.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[non_exhaustive]
pub struct Tag {
    pub slug: Slug,
    pub written: String,
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
    pub body: Document,
}

impl Page {
    /// A page with no date, no update and no tags; its id is derived from `file`.
    pub fn new(file: ContentPath, route: Route, kind: PageKind, title: impl Into<String>, body: Document) -> Self {
        let id = PageId::of(&file);
        Self { id, file, route, kind, title: title.into(), date: None, updated: None, tags: Vec::new(), body }
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

/// The site the engine interpreted. Backlinks, tag pages and feeds are functions of it, not
/// fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Site {
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
    pub fn new(root: Root, layout: Layout) -> Self {
        Self { root, layout, home: None, pages: Vec::new(), assets: Vec::new(), nav: Vec::new(), excluded: Vec::new() }
    }
}

impl Site {
    /// Pages and assets are sorted by route, exclusions by path. A route used twice, or a home
    /// or nav entry that is not a page, is an error.
    pub fn new(parts: SiteParts) -> Result<Self, SiteError> {
        let SiteParts { root, layout, home, mut pages, mut assets, nav, mut excluded } = parts;
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
        Ok(Self { root, layout, home, pages, assets, nav, excluded })
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
