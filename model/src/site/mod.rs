//! The interpreted site: pages with routes, assets, navigation, and what was left out and why.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

mod author;
pub(crate) mod document;
pub mod highlight;
mod page;
pub(crate) mod picture;
pub(crate) mod route;
mod tag;

pub use author::{Author, EmptyAuthor};
pub use page::{Page, PageId, PageKind};
pub use tag::{Tag, TagError};

use crate::{AnyCode, ContentPath, Hash, Language, Picture, Route};

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
    /// Dated posts in a collection, and pages beside them.
    Blog,
    /// A notes vault, such as Obsidian's: nothing is published until it is selected.
    Vault,
    /// Documentation, marked by a `docs` folder: pages, no posts.
    Docs,
    /// Pages and nothing else: the layout when nothing marks another.
    Flat,
}

/// A file published as is because a published page uses it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[non_exhaustive]
pub struct Asset {
    /// The file.
    pub file: ContentPath,
    /// Where it is served.
    pub route: Route,
    /// The hash of its bytes.
    pub blob: Hash,
    /// What the engine prepared from it when it is a raster image, once rendering has begun;
    /// `None` before, and for every other file.
    pub picture: Option<Picture>,
}

impl Asset {
    /// `file`, served at `route` with the bytes of `blob`, not yet prepared.
    pub fn new(file: ContentPath, route: Route, blob: Hash) -> Self {
        Self { file, route, blob, picture: None }
    }
}

/// A file left out of the site, and the decision that left it out.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[non_exhaustive]
pub struct Excluded {
    /// The file.
    pub path: ContentPath,
    /// The decision that left it out: `file.ignore`, `page.publish`, `site.root`.
    pub because: AnyCode,
}

impl Excluded {
    /// `path`, left out by the decision `because`.
    pub fn new(path: ContentPath, because: AnyCode) -> Self {
        Self { path, because }
    }
}

/// What a site says about itself: its name, its line, its authors, its language.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Identity {
    /// Its name.
    pub title: String,
    /// One line saying what it is, or `None`.
    pub description: Option<String>,
    /// First author first.
    pub authors: Vec<Author>,
    /// The language of its pages.
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
    /// Where each page is in `pages`, by id: a theme looks a page up for every link it writes.
    /// Built from `pages`, so it is not part of what the site is.
    #[serde(skip)]
    ids: BTreeMap<PageId, usize>,
}

/// Why parts do not make a [`Site`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SiteError {
    /// Two pages or assets share a route.
    Collision {
        /// The route.
        route: Route,
        /// Two of the files that route there.
        files: [ContentPath; 2],
    },
    /// The home or a nav entry names a page the site does not hold.
    Missing {
        /// The page named.
        page: PageId,
    },
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
    /// What the site says about itself.
    pub identity: Identity,
    /// Where its content starts.
    pub root: Root,
    /// How its files are arranged.
    pub layout: Layout,
    /// The home page, or `None` for a generated listing.
    pub home: Option<PageId>,
    /// Every page.
    pub pages: Vec<Page>,
    /// Every asset.
    pub assets: Vec<Asset>,
    /// The pages in the navigation, in order.
    pub nav: Vec<PageId>,
    /// Every file left out.
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
        let mut ids = BTreeMap::new();
        for (at, page) in pages.iter().enumerate() {
            // the first in route order, as a search through `pages` finds it
            ids.entry(page.id).or_insert(at);
        }
        if let Some(page) = home.iter().chain(&nav).find(|id| !ids.contains_key(*id)) {
            return Err(SiteError::Missing { page: *page });
        }
        Ok(Self { identity, root, layout, home, pages, assets, nav, excluded, ids })
    }

    /// What the site says about itself.
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// Where its content starts in the snapshot.
    pub fn root(&self) -> &Root {
        &self.root
    }

    /// How its files are arranged.
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

    /// The same site, each asset given what `prepared` says of it.
    #[must_use]
    pub fn pictured(mut self, mut prepared: impl FnMut(&Asset) -> Option<Picture>) -> Self {
        for asset in &mut self.assets {
            asset.picture = prepared(asset);
        }
        self
    }

    /// The page `id`, if the site holds it.
    pub fn page(&self, id: PageId) -> Option<&Page> {
        self.ids.get(&id).and_then(|&at| self.pages.get(at))
    }
}
