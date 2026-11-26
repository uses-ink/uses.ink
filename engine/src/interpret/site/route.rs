//! The stage giving routes, from decisions: the home is `/`; any other page is its folders'
//! slugs then its own slug, except a folder's index (`index` or `README` named by its file
//! name), which is its folder's route; a plain file keeps its folders and its name, each dotted
//! part slugged. Two files at one route, or a file at a route the engine serves, is
//! `page.route.collision`.

use std::collections::BTreeMap;

use model::code::Slug;
use model::{ContentPath, Diagnostic, Evidence, Problem, Route, Subject};

use crate::interpret::decide::Filed;
use crate::interpret::file::read::Source;
use crate::interpret::journal::Journal;
use crate::interpret::stage::Stage;
use crate::interpret::tree::InRoot;
use crate::render::Reserved;

/// Every published file's route.
#[derive(Debug, Default)]
pub(crate) struct Routes {
    pub(crate) pages: BTreeMap<ContentPath, Route>,
    pub(crate) assets: BTreeMap<ContentPath, Route>,
}

/// Routes every published page and plain file, then reports collisions.
#[derive(Debug)]
pub(crate) struct Routing<'a> {
    pub(crate) home: Option<&'a ContentPath>,
    /// The plain files published.
    pub(crate) assets: &'a [InRoot<'a>],
}

impl Stage<&[&Source]> for Routing<'_> {
    type Out = Routes;

    fn run(&self, pages: &[&Source], journal: &mut Journal) -> Routes {
        let mut routes = Routes::default();
        for source in pages {
            if let Some(route) = self.page(source, journal) {
                routes.pages.insert(source.path.clone(), route);
            }
        }
        for file in self.assets {
            if let Some(route) = Self::asset(file, journal) {
                routes.assets.insert(file.entry.path.clone(), route);
            }
        }
        Self::collisions(&routes, journal);
        routes
    }
}

impl Routing<'_> {
    fn page(&self, source: &Source, journal: &mut Journal) -> Option<Route> {
        if Some(&source.path) == self.home {
            return Some(Route::root());
        }
        let folder = Self::folder(source, journal);
        let subject = source.subject();
        let named = journal
            .decision::<Slug>(&subject)
            .is_some_and(|decision| matches!(decision.evidence, Evidence::Filename { .. }));
        let slug = journal.needed::<Slug>(&subject)?;
        let index = named && source.name().is_index() && !folder.is_root();
        Some(if index { folder } else { folder.child(&slug) })
    }

    fn asset(file: &InRoot<'_>, journal: &mut Journal) -> Option<Route> {
        let folder = Self::folder(file, journal);
        let parts = Self::slugs(file, file.relative.file_name().split('.'), journal);
        (!parts.is_empty()).then(|| folder.file(&parts))
    }

    /// The route of the folder holding `file`.
    fn folder(file: &impl Filed, journal: &mut Journal) -> Route {
        let parent = file.relative().parent();
        let segments = parent.iter().flat_map(ContentPath::segments);
        Route::from_slugs(&Self::slugs(file, segments, journal))
    }

    /// Each segment's slug; one with no letter or digit is left out and said so.
    fn slugs<'a>(
        file: &impl Filed,
        segments: impl IntoIterator<Item = &'a str>,
        journal: &mut Journal,
    ) -> Vec<model::Slug> {
        let mut slugs = Vec::new();
        for segment in segments {
            if let Ok(slug) = model::Slug::from_text(segment) {
                slugs.push(slug);
            } else {
                let path = file.path().clone();
                let problem = Problem::RouteSegmentUnnamed { path: path.clone(), segment: segment.to_owned() };
                journal.diagnose(Diagnostic::new(problem).on(Subject::File(path)));
            }
        }
        slugs
    }

    fn collisions(routes: &Routes, journal: &mut Journal) {
        let mut owners: BTreeMap<&Route, Vec<ContentPath>> = BTreeMap::new();
        for (file, route) in routes.pages.iter().chain(&routes.assets) {
            owners.entry(route).or_default().push(file.clone());
        }
        for (route, mut files) in owners {
            // `/404` is the author's own not-found page when a page takes it, never a plain file's.
            let plain = files.iter().any(|file| routes.assets.contains_key(file));
            let reserved = Reserved::ALL
                .iter()
                .any(|owned| owned.as_ref() == route.as_str() && (*owned != Reserved::NotFound || plain));
            if files.len() > 1 || reserved {
                files.sort();
                let problem = Problem::PageRouteCollision { route: route.clone(), files };
                journal.diagnose(Diagnostic::new(problem).on(Subject::Site));
            }
        }
    }
}
