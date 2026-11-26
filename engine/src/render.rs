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
mod xml;

use std::collections::BTreeMap;

use model::{Diagnostic, Hash, Manifest, Notice, Route, Site};

use crate::error::Error;
use crate::image::Images;
use crate::{Blobs, Engine, Theme};
use asset::Assets;
use feed::{Atom, Feed, Rss};
use output::Output;
use picture::Prepare;
use policy::Policy;
pub use settings::{BaseUrl, BaseUrlError, RenderSettings};
use sitemap::{Robots, Sitemap};
pub use theme::Generated;
use theme::{NotFound, Pages, ThemeFiles};

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
        let policy = match Policy::new() {
            Ok(policy) => policy,
            Err(error) => {
                let diagnostics = vec![error.into()];
                return Rendered {
                    manifest: Manifest::default(),
                    blobs: BTreeMap::new(),
                    diagnostics,
                    notices: vec![],
                };
            }
        };
        let mut out = Output::new(&policy, &settings.base);
        let pictures = Prepare { blobs: &self.blobs, images }.run(site, &mut out);
        let site = &site.clone().pictured(|asset| pictures.picture(asset));
        let theme = &self.theme;
        let artifacts = (
            Pages { theme },
            NotFound { theme },
            Assets { pictures: &pictures },
            Feed::<(Atom, Rss)>::default(),
            Sitemap,
            Robots,
            ThemeFiles { theme },
        );
        if let Err(error) = artifacts.files(site, &mut out) {
            out.fail(error);
        }
        out.finish()
    }
}
