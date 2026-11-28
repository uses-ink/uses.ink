//! What a render says of its units: what a theme reads, the unit a page is, what became of it,
//! and what a render takes from its host.

use std::fmt;

use model::ContentPath;
use serde::Serialize;

use crate::cache::Cache;
use crate::render::Generated;

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

impl Unit {
    /// The kind of `unit` span this is.
    pub(crate) const fn layer(&self) -> logging::Unit {
        match self {
            Self::Page(_) => logging::Unit::Page,
            Self::Generated(_) => logging::Unit::Generated,
        }
    }
}

impl logging::Kind for Outcome {
    fn kind(self) -> &'static str {
        match self {
            Self::Reused => "reused",
            Self::Rendered => "rendered",
        }
    }
}

/// Why the planner chose what it did for one unit. The cache is keyed by a digest of everything
/// the unit read, so the reason is how the lookup went, not which input changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reason {
    /// The cache held the unit under its key.
    Hit,
    /// The cache held nothing usable under the key: absent, or an entry that was not one page.
    Miss,
    /// No cache, or a theme with no identity to key by: every build renders.
    Unkeyed,
}

impl logging::Kind for Reason {
    fn kind(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Miss => "miss",
            Self::Unkeyed => "unkeyed",
        }
    }
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
