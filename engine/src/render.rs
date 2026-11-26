//! Render: a site becomes a manifest and the bytes behind it. The theme renders each page and
//! gives its own files; the engine owns everything else (decision 0014): routes, the feeds, the
//! sitemap, `robots.txt`, the not-found page's status, media types and every header.

mod feed;
mod header;
mod output;
mod settings;
mod xml;

use std::collections::BTreeMap;
use std::fmt;

use model::{
    Block, ContentPath, Diagnostic, Document, Hash, Inline, Manifest, MediaType, Notice, Page, PageKind, Problem,
    Route, Site, Status, Subject,
};

use crate::Theme;
use crate::error::Broken;
use header::Headers;
use output::{Body, Entry, Output, Owner};
pub use settings::{BaseUrl, BaseUrlError, RenderSettings};

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

/// The routes the engine serves itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Owned {
    Atom,
    Rss,
    Sitemap,
    Robots,
    /// The author's `/404` page, or the engine's own.
    NotFound,
}

impl Owned {
    pub(crate) const ALL: &'static [Self] = &[Self::Atom, Self::Rss, Self::Sitemap, Self::Robots, Self::NotFound];

    pub(crate) const fn written(self) -> &'static str {
        match self {
            Self::Atom => "/feed.xml",
            Self::Rss => "/rss.xml",
            Self::Sitemap => "/sitemap.xml",
            Self::Robots => "/robots.txt",
            Self::NotFound => "/404",
        }
    }

    fn route(self) -> Result<Route, Broken> {
        let written = self.written();
        written.parse().map_err(|source| Broken::Route { written, source })
    }
}

/// Renders `site` through `theme`.
pub fn render(site: &Site, theme: &dyn Theme, settings: &RenderSettings) -> Rendered {
    let headers = match Headers::new() {
        Ok(headers) => headers,
        Err(broken) => {
            let diagnostics = vec![broken.into()];
            return Rendered {
                manifest: Manifest::default(),
                blobs: BTreeMap::new(),
                diagnostics,
                notices: Vec::new(),
            };
        }
    };
    let mut out = Output::new(headers);
    let step = pages(site, theme, &mut out);
    out.settle(step);
    for generated in [Generated::Home, Generated::NotFound] {
        let step = generated.serve(site, theme, &mut out);
        out.settle(step);
    }
    assets(site, &mut out);
    let step = feeds(site, settings, &mut out);
    out.settle(step);
    let step = sitemap(site, settings, &mut out);
    out.settle(step);
    theme_files(theme, &mut out);
    out.finish()
}

/// Every page through the theme. The page at `/404`, if the author wrote one, is served as the
/// not-found page.
fn pages(site: &Site, theme: &dyn Theme, out: &mut Output) -> Result<(), Broken> {
    let not_found = Owned::NotFound.route()?;
    for page in site.pages() {
        let status = if page.route == not_found { Status::NotFound } else { Status::Ok };
        page_entry(page, site, theme, status, out);
    }
    Ok(())
}

fn page_entry(page: &Page, site: &Site, theme: &dyn Theme, status: Status, out: &mut Output) {
    match theme.render(page, site) {
        Ok(html) => out.serve(Entry {
            route: page.route.clone(),
            media: MediaType::Html,
            status,
            body: Body::Bytes(html.into()),
            owner: Owner::Page(page.file.clone()),
        }),
        Err(cause) => {
            let problem = Problem::ThemePageRefused { theme: theme.name().to_owned() };
            out.diagnose(Diagnostic::new(problem).on(Subject::File(page.file.clone())).caused_by(cause));
        }
    }
}

/// A page the engine makes when the author wrote none: the home, a listing the theme fills, and
/// the not-found page, served with status 404. Each is a page the theme renders like any other.
#[derive(Clone, Copy, Debug)]
enum Generated {
    /// At `/` when no page is the home (`site.home` decided a generated listing).
    Home,
    NotFound,
}

impl Generated {
    /// Serves this page unless a page of the site already has its route.
    fn serve(self, site: &Site, theme: &dyn Theme, out: &mut Output) -> Result<(), Broken> {
        let (file, route, kind, status) = match self {
            Self::Home => ("index", Route::root(), PageKind::Home, Status::Ok),
            Self::NotFound => ("404", Owned::NotFound.route()?, PageKind::Page, Status::NotFound),
        };
        if out.owner(&route).is_some() || (matches!(self, Self::Home) && site.home().is_some()) {
            return Ok(());
        }
        let file: ContentPath = file.parse().map_err(|source| Broken::Path { written: file, source })?;
        let page = match self {
            Self::Home => Page::new(file, route, kind, site.identity().title.clone(), Document::default()),
            Self::NotFound => {
                let body = Document::new(vec![Block::Paragraph(vec![Inline::Text(NotFound::Body.to_string())])]);
                Page::new(file, route, kind, NotFound::Title.to_string(), body)
            }
        };
        page_entry(&page, site, theme, status, out);
        Ok(())
    }
}

/// The words of the engine's not-found page.
#[derive(Clone, Copy, Debug)]
enum NotFound {
    Title,
    Body,
}

impl fmt::Display for NotFound {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Title => "Page not found",
            Self::Body => "There is no page at this address.",
        })
    }
}

/// Plain files a published page uses, from the snapshot's own blobs.
fn assets(site: &Site, out: &mut Output) {
    for asset in site.assets() {
        let Some(media) = media(&asset.file) else {
            let problem = Problem::AssetMediaUnknown { path: asset.file.clone() };
            out.diagnose(Diagnostic::new(problem).on(Subject::File(asset.file.clone())));
            continue;
        };
        out.serve(Entry {
            route: asset.route.clone(),
            media,
            status: Status::Ok,
            body: Body::Blob(asset.blob),
            owner: Owner::Asset(asset.file.clone()),
        });
    }
}

/// The media type of a plain file, by its extension.
fn media(path: &ContentPath) -> Option<MediaType> {
    let extension = path.extension()?.to_ascii_lowercase();
    Some(match extension.as_str() {
        "png" => MediaType::Png,
        "jpg" | "jpeg" => MediaType::Jpeg,
        "avif" => MediaType::Avif,
        "webp" => MediaType::Webp,
        "gif" => MediaType::Gif,
        "svg" => MediaType::Svg,
        "ico" => MediaType::Ico,
        "pdf" => MediaType::Pdf,
        _ => return None,
    })
}

fn feeds(site: &Site, settings: &RenderSettings, out: &mut Output) -> Result<(), Broken> {
    let Some(channel) = feed::channel(site, settings, &mut out.notices)? else { return Ok(()) };
    for (owned, media) in [(Owned::Atom, MediaType::Atom), (Owned::Rss, MediaType::Rss)] {
        let route = owned.route()?;
        let own = settings.base.at(&route);
        let written = match owned {
            Owned::Atom => xml::atom(&channel, &own),
            _ => xml::rss(&channel, &own),
        };
        let bytes = written.map_err(|source| Broken::Xml { route: route.clone(), source })?;
        out.serve(Entry { route, media, status: Status::Ok, body: Body::Bytes(bytes), owner: Owner::Engine });
    }
    Ok(())
}

/// The sitemap of every page but the not-found page, a generated home among them, and
/// `robots.txt` pointing at it.
fn sitemap(site: &Site, settings: &RenderSettings, out: &mut Output) -> Result<(), Broken> {
    let not_found = Owned::NotFound.route()?;
    let generated =
        site.home().is_none().then(|| xml::Location { url: settings.base.at(&Route::root()), changed: None });
    let locations: Vec<xml::Location> = generated
        .into_iter()
        .chain(site.pages().iter().filter(|page| page.route != not_found).map(|page| xml::Location {
            url: settings.base.at(&page.route),
            changed: page.updated.or(page.date).map(model::When::day),
        }))
        .collect();
    let route = Owned::Sitemap.route()?;
    let bytes = xml::sitemap(&locations).map_err(|source| Broken::Xml { route: route.clone(), source })?;
    let sitemap = settings.base.at(&route);
    out.serve(Entry {
        route,
        media: MediaType::Xml,
        status: Status::Ok,
        body: Body::Bytes(bytes),
        owner: Owner::Engine,
    });
    let robots = Robots { sitemap: &sitemap }.to_string().into_bytes();
    let route = Owned::Robots.route()?;
    out.serve(Entry {
        route,
        media: MediaType::Text,
        status: Status::Ok,
        body: Body::Bytes(robots),
        owner: Owner::Engine,
    });
    Ok(())
}

/// `robots.txt`: everything may be crawled; the sitemap says what there is.
#[derive(Debug)]
struct Robots<'a> {
    sitemap: &'a str,
}

impl fmt::Display for Robots<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(out, "User-agent: *")?;
        writeln!(out, "Allow: /")?;
        writeln!(out)?;
        writeln!(out, "Sitemap: {}", self.sitemap)
    }
}

/// The theme's files, last: a route the site or the engine already serves is refused.
fn theme_files(theme: &dyn Theme, out: &mut Output) {
    for file in theme.assets() {
        let refused = match out.owner(&file.route) {
            None => None,
            Some(Owner::Page(path) | Owner::Asset(path)) => Some(Problem::ThemeRouteCollision {
                route: file.route.clone(),
                theme: theme.name().to_owned(),
                file: path.clone(),
            }),
            Some(Owner::Engine | Owner::Theme) => {
                Some(Problem::ThemeRouteReserved { route: file.route.clone(), theme: theme.name().to_owned() })
            }
        };
        match refused {
            Some(problem) => out.diagnose(Diagnostic::new(problem)),
            None => out.serve(Entry {
                route: file.route,
                media: file.media,
                status: Status::Ok,
                body: Body::Bytes(file.bytes),
                owner: Owner::Theme,
            }),
        }
    }
}
