//! Routes, from decisions: the home is `/`; any other page is its folders' slugs then its own
//! slug, except a folder's index (`index` or `README` named by its file name), which is its
//! folder's route; a plain file keeps its folders and its name, each dotted part slugged. Two
//! files at one route, or a file at a route the engine serves, is `page.route.collision`.

use std::collections::BTreeMap;

use model::{ContentPath, Diagnostic, Evidence, Problem, Route, Slug, Subject};

use crate::interpret::file::read::Source;
use crate::interpret::file::scope::InRoot;
use crate::interpret::ladder::code;
use crate::interpret::record::Record;
use crate::render::Owned;

/// Every published file's route.
#[derive(Debug, Default)]
pub(crate) struct Routes {
    pub(crate) pages: BTreeMap<ContentPath, Route>,
    pub(crate) assets: BTreeMap<ContentPath, Route>,
}

/// Routes every published page and plain file, then reports collisions.
pub(crate) fn assign(
    pages: &[&Source],
    assets: &[InRoot<'_>],
    home: Option<&ContentPath>,
    record: &mut Record,
) -> Routes {
    let mut routes = Routes::default();
    for source in pages {
        if let Some(route) = page(source, home, record) {
            routes.pages.insert(source.path.clone(), route);
        }
    }
    for file in assets {
        if let Some(route) = asset(file, record) {
            routes.assets.insert(file.path().clone(), route);
        }
    }
    collisions(&routes, record);
    routes
}

fn page(source: &Source, home: Option<&ContentPath>, record: &mut Record) -> Option<Route> {
    if Some(&source.path) == home {
        return Some(Route::root());
    }
    let folder = folder(&source.path, &source.relative, record);
    let subject = source.subject();
    let named = record
        .decision::<code::Slug>(&subject)
        .is_some_and(|decision| matches!(decision.evidence, Evidence::Filename { .. }));
    let slug = record.needed::<code::Slug>(&subject)?;
    let index = named && source.name().is_index() && !folder.is_root();
    Some(if index { folder } else { folder.child(&slug) })
}

fn asset(file: &InRoot<'_>, record: &mut Record) -> Option<Route> {
    let folder = folder(file.path(), &file.relative, record);
    let parts = slugs(file.path(), file.relative.file_name().split('.'), record);
    (!parts.is_empty()).then(|| folder.file(&parts))
}

/// The route of the folder holding `relative`.
fn folder(path: &ContentPath, relative: &ContentPath, record: &mut Record) -> Route {
    let parent = relative.parent();
    let segments = parent.iter().flat_map(ContentPath::segments);
    Route::from_slugs(&slugs(path, segments, record))
}

/// Each segment's slug; one with no letter or digit is left out and said so.
fn slugs<'a>(path: &ContentPath, segments: impl Iterator<Item = &'a str>, record: &mut Record) -> Vec<Slug> {
    let mut slugs = Vec::new();
    for segment in segments {
        if let Ok(slug) = Slug::from_text(segment) {
            slugs.push(slug);
        } else {
            let problem = Problem::RouteSegmentUnnamed { path: path.clone(), segment: segment.to_owned() };
            record.diagnose(Diagnostic::new(problem).on(Subject::File(path.clone())));
        }
    }
    slugs
}

fn collisions(routes: &Routes, record: &mut Record) {
    let mut owners: BTreeMap<&Route, Vec<ContentPath>> = BTreeMap::new();
    for (file, route) in routes.pages.iter().chain(&routes.assets) {
        owners.entry(route).or_default().push(file.clone());
    }
    for (route, mut files) in owners {
        // `/404` is the author's own not-found page when a page takes it, never a plain file's.
        let plain = files.iter().any(|file| routes.assets.contains_key(file));
        let reserved =
            Owned::ALL.iter().any(|owned| owned.written() == route.as_str() && (*owned != Owned::NotFound || plain));
        if files.len() > 1 || reserved {
            files.sort();
            let problem = Problem::PageRouteCollision { route: route.clone(), files };
            record.diagnose(Diagnostic::new(problem).on(Subject::Site));
        }
    }
}
