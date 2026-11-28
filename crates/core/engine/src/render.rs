//! Render: a site becomes a manifest and the bytes behind it, through a list of artifacts, each
//! a type producing served files. The theme renders each page and gives its own files; the
//! engine owns everything else (decision 0014): routes, the feeds, the sitemap, `robots.txt`,
//! the not-found page's status, media types and, through one [`Policy`], every header.

mod asset;
mod feed;
mod output;
mod picture;
mod policy;
mod settings;
mod sitemap;
mod theme;
mod unit;
mod xml;

use std::borrow::Cow;
use std::collections::BTreeMap;

use model::{Announcement, Diagnostic, Hash, Manifest, Notice, Route, Site};

use crate::cache::Cache;
use crate::error::Error;
use crate::image::Images;
use crate::{Blobs, Engine, Sink, Theme};
use asset::Assets;
use feed::{Atom, Feed, Rss};
use output::Output;
use picture::Prepare;
use policy::Policy;
pub use settings::{BaseUrl, BaseUrlError, RenderSettings};
use sitemap::{Robots, Sitemap};
pub use theme::Generated;
use theme::{NotFound, Pages, ThemeFiles};
use unit::Units;
pub use unit::{Host, Outcome, Reads, Step, Unit};

/// What a render produced.
#[derive(Debug)]
#[non_exhaustive]
pub struct Rendered {
    /// Every route served. Empty when the engine's own headers could not be built, so nothing is
    /// ever served without them.
    pub manifest: Manifest,
    /// The bytes of every blob the render made, by hash. Plain files of the snapshot are served
    /// from the blobs the snapshot already stored, so they are not here.
    pub blobs: BTreeMap<Hash, Vec<u8>>,
    /// An error here means the deploy must not be published (all or nothing).
    pub diagnostics: Vec<Diagnostic>,
    /// What is worth knowing: feeds left out, posts not in them.
    pub notices: Vec<Notice>,
    /// What became of each page the theme was asked for, in the order they were served: taken
    /// from the cache, or rendered.
    pub steps: Vec<Step>,
}

impl Rendered {
    /// A render that could not start: nothing is served, and `error` says why.
    fn refused(error: Error) -> Self {
        Self {
            manifest: Manifest::default(),
            blobs: BTreeMap::new(),
            diagnostics: vec![error.into()],
            notices: vec![],
            steps: vec![],
        }
    }
}

/// One output of a render: files it serves for `site`. A tuple of artifacts is an artifact,
/// each producing its files in order; one that fails is recorded and the next still runs.
pub(crate) trait Artifact {
    /// Serves this artifact's files into `out`.
    fn files(&self, site: &Site, out: &mut Output<'_>) -> Result<(), Error>;
}

/// Implements [`Artifact`] for a tuple of artifacts, run in order. The same impl over tuples of
/// every length up to eight, which a generic cannot write.
///
/// ```text
/// artifacts!(A 0, B 1);
/// // makes `impl<A: Artifact, B: Artifact> Artifact for (A, B)`
/// ```
macro_rules! artifacts {
    ($($artifact:ident $at:tt),+) => {
        impl<$($artifact: Artifact),+> Artifact for ($($artifact,)+) {
            fn files(&self, site: &Site, out: &mut Output<'_>) -> Result<(), Error> {
                $(
                    if let Err(error) = self.$at.files(site, out) {
                        out.fail(error);
                    }
                )+
                Ok(())
            }
        }
    };
}

artifacts!(A 0);
artifacts!(A 0, B 1);
artifacts!(A 0, B 1, C 2);
artifacts!(A 0, B 1, C 2, D 3);
artifacts!(A 0, B 1, C 2, D 3, E 4);
artifacts!(A 0, B 1, C 2, D 3, E 4, F 5);
artifacts!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
artifacts!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);

/// The routes the engine serves itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Reserved {
    /// The Atom feed.
    Atom,
    /// The RSS feed.
    Rss,
    /// The sitemap.
    Sitemap,
    /// `robots.txt`.
    Robots,
    /// The author's `/404` page, or the engine's own.
    NotFound,
}

impl Reserved {
    /// Every route the engine serves itself.
    pub(crate) const ALL: &'static [Self] = &[Self::Atom, Self::Rss, Self::Sitemap, Self::Robots, Self::NotFound];

    /// The route.
    pub(crate) fn route(self) -> Result<Route, Error> {
        self.as_ref().parse().map_err(|source| Error::Route { reserved: self, source })
    }
}

impl AsRef<str> for Reserved {
    /// The route as written.
    fn as_ref(&self) -> &str {
        match self {
            Self::Atom => "/feed.xml",
            Self::Rss => "/rss.xml",
            Self::Sitemap => "/sitemap.xml",
            Self::Robots => "/robots.txt",
            Self::NotFound => "/404",
        }
    }
}

impl<B: Blobs, P, F, T: Theme> Engine<B, P, F, T> {
    /// Renders `site` through the theme: first its raster images through `images`, so each
    /// page knows their sizes and copies; then its pages, a generated home and not-found page
    /// when the site has none, the plain files pages use, the feeds, the sitemap, `robots.txt`,
    /// then the theme's own files, which may take no route already served.
    pub fn render<I: Images>(&self, site: &Site, settings: &RenderSettings, images: &I) -> Rendered {
        self.render_with(site, settings, Host::new(images))
    }

    /// As [`Engine::render`], taking each page from the host's cache when the cache holds it
    /// under the key of everything the page was made from, and keeping it there when not
    /// (decision 0048). What the render gives is the same, byte for byte, with a cache, a cold
    /// one, or none.
    pub fn render_with<I: Images, C: Cache>(
        &self,
        site: &Site,
        settings: &RenderSettings,
        host: Host<'_, I, C>,
    ) -> Rendered {
        let images = host.images;
        let policy = match Policy::new() {
            Ok(policy) => policy,
            Err(error) => return Rendered::refused(error),
        };
        let mut out = Output::new(&policy, &settings.base);
        let pictures = Prepare { blobs: &self.blobs, images }.run(site, &mut out);
        // A copy of every page only when a picture changes an asset: a site without raster
        // images is rendered as interpreted.
        let unchanged = site.assets().iter().all(|asset| pictures.picture(asset) == asset.picture);
        let site: Cow<'_, Site> = if unchanged {
            Cow::Borrowed(site)
        } else {
            Cow::Owned(site.clone().pictured(|asset| pictures.picture(asset)))
        };
        let site = &*site;
        let theme = &self.theme;
        let units = Units::new(host.cache(), theme.identity(), &settings.base, site);
        let artifacts = (
            Pages { theme, units: &units },
            NotFound { theme, units: &units },
            Assets { pictures: &pictures },
            Feed::<(Atom, Rss)>::default(),
            Sitemap,
            Robots,
            ThemeFiles { theme },
        );
        let stage = logging::stage!(logging::Stage::Render, "artifacts").entered();
        if let Err(error) = artifacts.files(site, &mut out) {
            out.fail(error);
        }
        drop(stage);
        if units.is_unkeyed() {
            out.notice(Notice::new(Announcement::RenderCacheUnkeyed { theme: theme.name().to_owned() }));
        } else if units.is_keyed() {
            let (reused, rendered) = out.tally();
            out.notice(Notice::new(Announcement::RenderCacheUsed { reused, rendered }));
        }
        out.finish()
    }
}
