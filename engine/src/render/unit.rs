//! The render unit index (decision 0045): which pages a render makes again and which it takes
//! from a [`Cache`]. A page is one unit. Its key is the hash of everything the theme may read
//! to write it: the engine's release, the theme's own identity, where the site is served, the
//! page, and the part of the site the theme says it reads ([`Reads`]). A key never holds a
//! time, so a build that changes nothing a page reads takes the page from the cache, and a
//! build with a cache gives the bytes a build without one gives.

use std::cell::{OnceCell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use model::{Asset, Block, ContentPath, Hash, ImageSource, Inline, LinkTarget, Page, PageId, Site, Visit};
use serde::Serialize;

use crate::RELEASE;
use crate::cache::{Cache, Key, Kind};
use crate::render::Generated;
use crate::render::feed::Channel;
use crate::render::settings::BaseUrl;

/// What a theme reads of the site while it writes one page, beyond the page itself. It is part
/// of the theme's contract: a theme that reads more than it says gets stale pages from a warm
/// build, so a theme that is not sure says [`Reads::Whole`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Reads {
    /// The site's identity, root, layout and home; its navigation, whether it has feeds, the
    /// pages the page links to or embeds, each as a page without its body (route, title, kind,
    /// dates, tags, summary); and the assets the page shows.
    Linked,
    /// What [`Reads::Linked`] says, and every page of the site without its body: a listing.
    Listed,
    /// Anything of the site. What a theme that says nothing reads.
    Whole,
}

/// A page one render made or took.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unit {
    /// A page of the site, by its file.
    Page(ContentPath),
    /// A page the engine made when the author wrote none.
    Generated(Generated),
}

impl fmt::Display for Unit {
    /// The file the page is read from, or the file a generated page stands for.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Page(file) => fmt::Display::fmt(file, out),
            Self::Generated(generated) => out.write_str(generated.as_ref()),
        }
    }
}

/// What became of a unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// Taken from the cache: the theme was not asked.
    Reused,
    /// Written by the theme: no cache, no entry, an entry that did not check out, a theme that
    /// does not identify itself, or a page the theme had something to say about.
    Rendered,
}

/// One unit and what became of it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Step {
    /// The unit.
    pub unit: Unit,
    /// What became of it.
    pub outcome: Outcome,
}

impl Step {
    /// `outcome` for `unit`.
    pub const fn new(unit: Unit, outcome: Outcome) -> Self {
        Self { unit, outcome }
    }
}

/// What a render takes from its host: the image work, and perhaps a cache to keep pages in.
#[derive(Debug)]
pub struct Host<'a, I, C> {
    /// Prepares the site's raster images.
    pub images: &'a I,
    cache: Option<&'a C>,
}

impl<I, C> Clone for Host<'_, I, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<I, C> Copy for Host<'_, I, C> {}

impl<'a, I> Host<'a, I, ()> {
    /// The host that prepares images with `images` and keeps nothing between builds.
    pub const fn new(images: &'a I) -> Self {
        Self { images, cache: None }
    }

    /// The same host, taking and keeping pages in `cache`.
    pub const fn cached<C: Cache>(self, cache: &'a C) -> Host<'a, I, C> {
        Host { images: self.images, cache: Some(cache) }
    }
}

impl<'a, I, C> Host<'a, I, C> {
    /// The cache, when there is one.
    pub(crate) const fn cache(&self) -> Option<&'a C> {
        self.cache
    }
}

/// A hash of what serde writes for a value: the one way a key is made.
struct Digest;

impl Digest {
    fn of<T: Serialize + ?Sized>(value: &T) -> Result<Hash, serde_json::Error> {
        serde_json::to_vec(value).map(Hash::of)
    }
}

/// A page without its body, as a theme that reads its neighbours sees it. Written from the
/// page's own serialization, so a field added to [`Page`] is in it without a line here.
struct Brief;

impl Brief {
    fn of(page: &Page) -> Result<Hash, serde_json::Error> {
        let mut written = serde_json::to_value(page)?;
        if let Some(fields) = written.as_object_mut() {
            fields.remove("body");
        }
        Digest::of(&written)
    }
}

/// The pages and assets a page's body points at.
#[derive(Debug, Default)]
struct Reached {
    pages: BTreeSet<PageId>,
    files: BTreeSet<ContentPath>,
}

impl Reached {
    fn of(page: &Page) -> Self {
        let mut reached = Self::default();
        page.body.walk(&mut reached);
        reached
    }

    fn target(&mut self, target: &LinkTarget) {
        if let LinkTarget::Page { id, .. } = target {
            self.pages.insert(*id);
        }
    }
}

impl Visit for Reached {
    fn block(&mut self, block: &Block) {
        if let Block::Embed { target, .. } = block {
            self.target(target);
        }
        block.walk(self);
    }

    fn inline(&mut self, inline: &Inline) {
        if let Inline::Embed { target } = inline {
            self.target(target);
        }
        inline.walk(self);
    }

    fn link(&mut self, target: &LinkTarget, children: &[Inline]) {
        self.target(target);
        self.inlines(children);
    }

    fn image(&mut self, source: &ImageSource, _alt: &str) {
        if let ImageSource::Asset(path) = source {
            self.files.insert(path.clone());
        }
    }
}

/// What a page's key holds besides the page.
#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
enum Context<'a> {
    Linked(Linked<'a>),
    Listed { frame: Hash, linked: Linked<'a> },
    Whole(Hash),
}

/// What [`Reads::Linked`] says.
#[derive(Serialize)]
struct Linked<'a> {
    site: Hash,
    pages: Vec<(PageId, Option<Hash>)>,
    assets: Vec<&'a Asset>,
}

/// What the same for every page of a site: its identity, root, layout, home and navigation, and
/// whether it has feeds.
#[derive(Serialize)]
struct Shared<'a> {
    identity: &'a model::Identity,
    root: &'a model::Root,
    layout: model::Layout,
    home: Option<PageId>,
    nav: Vec<(PageId, Option<Hash>)>,
    feeds: bool,
}

/// What a page's key is made of.
#[derive(Serialize)]
struct Keyed<'a> {
    engine: &'static str,
    theme: Hash,
    base: &'a str,
    reads: Reads,
    page: &'a Page,
    context: Context<'a>,
}

/// The render unit index of one render: keys for its pages, and the cache they are kept in.
#[derive(Debug)]
pub(crate) struct Units<'a, C> {
    cache: Option<&'a C>,
    /// The theme's identity; `None` when it has none, so no page has a key.
    theme: Option<Hash>,
    base: &'a BaseUrl,
    site: &'a Site,
    briefs: RefCell<BTreeMap<PageId, Hash>>,
    shared: OnceCell<Hash>,
    frame: OnceCell<Hash>,
    world: OnceCell<Hash>,
    assets: OnceCell<BTreeMap<&'a ContentPath, &'a Asset>>,
}

impl<'a, C: Cache> Units<'a, C> {
    /// The index of a render of `site` under `base`, by a theme with identity `theme`.
    pub(crate) fn new(cache: Option<&'a C>, theme: Option<Hash>, base: &'a BaseUrl, site: &'a Site) -> Self {
        Self {
            cache,
            theme,
            base,
            site,
            briefs: RefCell::default(),
            shared: OnceCell::new(),
            frame: OnceCell::new(),
            world: OnceCell::new(),
            assets: OnceCell::new(),
        }
    }

    /// Whether a cache is in use but the theme gives no identity to key it by.
    pub(crate) const fn is_unkeyed(&self) -> bool {
        self.cache.is_some() && self.theme.is_none()
    }

    /// Whether pages are taken from and kept in a cache.
    pub(crate) const fn is_keyed(&self) -> bool {
        self.cache.is_some() && self.theme.is_some()
    }

    /// The key of `page` as a theme that reads `reads` writes it; `None` when nothing is kept.
    pub(crate) fn key(&self, page: &Page, reads: Reads) -> Result<Option<Key>, serde_json::Error> {
        let (Some(_), Some(theme)) = (self.cache, self.theme) else { return Ok(None) };
        let context = match reads {
            Reads::Linked => Context::Linked(self.linked(page)?),
            Reads::Listed => Context::Listed { frame: self.frame()?, linked: self.linked(page)? },
            Reads::Whole => Context::Whole(self.world()?),
        };
        let keyed = Keyed { engine: RELEASE, theme, base: self.base.as_ref(), reads, page, context };
        Digest::of(&keyed).map(|digest| Some(Key::new(Kind::Page, digest)))
    }

    /// The page `key` names, if the cache holds it and it checks out; an entry that is not
    /// exactly one value is dropped, and the cache says so.
    pub(crate) fn take(&self, key: &Key) -> Option<Vec<u8>> {
        let cache = self.cache?;
        let mut values = cache.get(key)?;
        if let (Some(html), true) = (values.pop(), values.is_empty()) {
            return Some(html);
        }
        cache.reject(key);
        None
    }

    /// Keeps `html` under `key`.
    pub(crate) fn keep(&self, key: &Key, html: &[u8]) {
        if let Some(cache) = self.cache {
            cache.put(key, &[html]);
        }
    }

    fn brief(&self, id: PageId) -> Result<Option<Hash>, serde_json::Error> {
        if let Some(hash) = self.briefs.borrow().get(&id) {
            return Ok(Some(*hash));
        }
        let Some(page) = self.site.page(id) else { return Ok(None) };
        let hash = Brief::of(page)?;
        self.briefs.borrow_mut().insert(id, hash);
        Ok(Some(hash))
    }

    fn briefs_of(
        &self,
        ids: impl IntoIterator<Item = PageId>,
    ) -> Result<Vec<(PageId, Option<Hash>)>, serde_json::Error> {
        ids.into_iter().map(|id| Ok((id, self.brief(id)?))).collect()
    }

    fn linked(&self, page: &Page) -> Result<Linked<'a>, serde_json::Error> {
        let reached = Reached::of(page);
        let assets = self.assets.get_or_init(|| self.site.assets().iter().map(|asset| (&asset.file, asset)).collect());
        Ok(Linked {
            site: self.shared()?,
            pages: self.briefs_of(reached.pages)?,
            assets: reached.files.iter().filter_map(|file| assets.get(file).copied()).collect(),
        })
    }

    fn shared(&self) -> Result<Hash, serde_json::Error> {
        if let Some(hash) = self.shared.get() {
            return Ok(*hash);
        }
        let site = self.site;
        let shared = Shared {
            identity: site.identity(),
            root: site.root(),
            layout: site.layout(),
            home: site.home().map(|page| page.id),
            nav: self.briefs_of(site.nav().iter().copied())?,
            feeds: Channel::is_dated(site),
        };
        let digest = Digest::of(&shared)?;
        Ok(*self.shared.get_or_init(|| digest))
    }

    /// Every page without its body, in the site's order.
    fn frame(&self) -> Result<Hash, serde_json::Error> {
        if let Some(hash) = self.frame.get() {
            return Ok(*hash);
        }
        let all = self.briefs_of(self.site.pages().iter().map(|page| page.id))?;
        let digest = Digest::of(&all)?;
        Ok(*self.frame.get_or_init(|| digest))
    }

    /// The whole site.
    fn world(&self) -> Result<Hash, serde_json::Error> {
        if let Some(hash) = self.world.get() {
            return Ok(*hash);
        }
        let digest = Digest::of(self.site)?;
        Ok(*self.world.get_or_init(|| digest))
    }
}
