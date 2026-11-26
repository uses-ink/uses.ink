//! What goes through the theme: every page ([`Pages`]), a generated home and not-found page
//! when the author wrote none ([`NotFound`]), and the theme's own files ([`ThemeFiles`]).

use std::fmt;

use model::{Block, ContentPath, Diagnostic, Document, Inline, MediaType, Page, PageKind, Problem, Route, Site};
use model::{Status, Subject};

use crate::error::Error;
use crate::render::output::{Body, Entry, Output, Owner};
use crate::render::{Artifact, Reserved};
use crate::{Record, Theme};

/// A page the engine makes when the author wrote none, rendered by the theme like any other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Generated {
    /// At `/` when no page is the home (`site.home` decided a generated listing).
    Home,
    /// At `/404`, served with status 404.
    NotFound,
}

impl Generated {
    /// The page, for `site`.
    fn page(self, site: &Site, route: Route) -> Result<Page, Error> {
        let file: ContentPath = self.as_ref().parse().map_err(|source| Error::Path { page: self, source })?;
        Ok(match self {
            Self::Home => Page::new(file, route, PageKind::Home, site.identity().title.clone()),
            Self::NotFound => {
                let body = Document::new(vec![Block::Paragraph(vec![Inline::Text(Missing::Body.to_string())])]);
                Page::new(file, route, PageKind::Page, Missing::Title.to_string()).holding(body)
            }
        })
    }
}

impl AsRef<str> for Generated {
    /// The file the page stands for.
    fn as_ref(&self) -> &str {
        match self {
            Self::Home => "index",
            Self::NotFound => "404",
        }
    }
}

/// The words of the engine's not-found page.
#[derive(Clone, Copy, Debug)]
enum Missing {
    Title,
    Body,
}

impl fmt::Display for Missing {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Title => "Page not found",
            Self::Body => "There is no page at this address.",
        })
    }
}

/// Every page through the theme, then a generated home when no page is the home. The page at
/// `/404`, if the author wrote one, is served as the not-found page.
#[derive(Debug)]
pub(crate) struct Pages<'t, T> {
    pub(crate) theme: &'t T,
}

impl<T: Theme> Pages<'_, T> {
    /// Serves `page` as the theme renders it, recording what the theme says on the way; the
    /// page at `/404` is served with status 404. A page the theme refuses is
    /// `theme.page.refused`, caused by the theme's diagnostic.
    fn serve(&self, page: &Page, site: &Site, out: &mut Output<'_>) -> Result<(), Error> {
        let status = if page.route == Reserved::NotFound.route()? { Status::NotFound } else { Status::Ok };
        match self.theme.render(page, site, out) {
            Ok(html) => out.serve(Entry {
                route: page.route.clone(),
                media: MediaType::Html,
                status,
                body: Body::Bytes(html.into()),
                owner: Owner::Page(page.file.clone()),
            }),
            Err(cause) => {
                let problem = Problem::ThemePageRefused { theme: self.theme.name().to_owned() };
                out.diagnose(Diagnostic::new(problem).on(Subject::File(page.file.clone())).caused_by(cause));
            }
        }
        Ok(())
    }
}

impl<T: Theme> Artifact for Pages<'_, T> {
    fn files(&self, site: &Site, out: &mut Output<'_>) -> Result<(), Error> {
        for page in site.pages() {
            self.serve(page, site, out)?;
        }
        let root = Route::root();
        if site.home().is_none() && out.owner(&root).is_none() {
            self.serve(&Generated::Home.page(site, root)?, site, out)?;
        }
        Ok(())
    }
}

/// The engine's not-found page, when no page of the site is at `/404`.
#[derive(Debug)]
pub(crate) struct NotFound<'t, T> {
    pub(crate) theme: &'t T,
}

impl<T: Theme> Artifact for NotFound<'_, T> {
    fn files(&self, site: &Site, out: &mut Output<'_>) -> Result<(), Error> {
        let route = Reserved::NotFound.route()?;
        if out.owner(&route).is_some() {
            return Ok(());
        }
        let page = Generated::NotFound.page(site, route)?;
        Pages { theme: self.theme }.serve(&page, site, out)?;
        Ok(())
    }
}

/// The theme's files, last: a route the site or the engine already serves is refused.
#[derive(Debug)]
pub(crate) struct ThemeFiles<'t, T> {
    pub(crate) theme: &'t T,
}

impl<T: Theme> Artifact for ThemeFiles<'_, T> {
    fn files(&self, _site: &Site, out: &mut Output<'_>) -> Result<(), Error> {
        let theme = self.theme.name();
        for file in self.theme.assets() {
            let refused = match out.owner(&file.route) {
                None => None,
                Some(Owner::Page(path) | Owner::Asset(path)) => Some(Problem::ThemeRouteCollision {
                    route: file.route.clone(),
                    theme: theme.to_owned(),
                    file: path.clone(),
                }),
                Some(Owner::Engine | Owner::Theme) => {
                    Some(Problem::ThemeRouteReserved { route: file.route.clone(), theme: theme.to_owned() })
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
        Ok(())
    }
}
