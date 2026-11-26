//! The site, built only from decisions: each value is read back from the record, so nothing in
//! the site lacks a cause.

use model::{
    Asset, ContentPath, DecisionCode, Diagnostic, Excluded, Identity, Page, PageId, PageKind, Problem, Site, SiteError,
    SiteParts, Subject,
};

use crate::error::Broken;
use crate::interpret::file::read::Source;
use crate::interpret::file::scope::{InRoot, Scope};
use crate::interpret::ladder::code;
use crate::interpret::record::Record;
use crate::interpret::site::route::Routes;

/// What the site is assembled from, besides the record.
#[derive(Debug)]
pub(crate) struct Parts<'a> {
    pub(crate) scope: &'a Scope<'a>,
    /// Every file a format read, its links resolved.
    pub(crate) sources: Vec<Source>,
    /// The plain files published.
    pub(crate) assets: &'a [InRoot<'a>],
    pub(crate) routes: &'a Routes,
}

pub(crate) fn site(parts: Parts<'_>, record: &mut Record) -> Option<Site> {
    let Parts { scope, sources, assets, routes } = parts;
    let layout = record.needed::<code::Layout>(&Subject::Site)?;
    let home = record.needed::<code::Home>(&Subject::Site)?;
    let nav = record.needed::<code::Nav>(&Subject::Site)?;
    let mut identity = Identity::new(
        record.needed::<code::SiteTitle>(&Subject::Site)?,
        record.needed::<code::Language>(&Subject::Site)?,
    );
    identity.description = record.needed::<code::Description>(&Subject::Site)?;
    identity.authors = record.needed::<code::Authors>(&Subject::Site)?;
    let mut site = SiteParts::new(identity, scope.root.clone(), layout);
    site.home = home.as_ref().map(PageId::of);
    site.nav = nav.iter().map(PageId::of).collect();
    site.excluded = excluded(scope, record);
    for source in sources {
        if record.get::<code::Publish>(&source.subject()) == Some(&true) {
            site.pages.push(page(source, routes, home.as_ref(), record)?);
        }
    }
    // A plain file without a route already said so (`route.segment.unnamed`).
    for file in assets {
        if let Some(route) = routes.assets.get(file.path()) {
            site.assets.push(Asset::new(file.path().clone(), route.clone(), file.entry.blob));
        }
    }
    let home_id = site.home;
    Site::new(site).map_err(|error| record.diagnose(diagnostic(error, home_id))).ok()
}

fn page(source: Source, routes: &Routes, home: Option<&ContentPath>, record: &mut Record) -> Option<Page> {
    let subject = source.subject();
    let route = routes.pages.get(&source.path)?.clone();
    let kind = if Some(&source.path) == home { PageKind::Home } else { record.needed::<code::Kind>(&subject)? };
    let title = record.needed::<code::Title>(&subject)?;
    let date = record.needed::<code::Date>(&subject)?;
    let updated = record.needed::<code::Updated>(&subject)?;
    let tags = record.needed::<code::Tags>(&subject)?;
    let summary = record.needed::<code::Summary>(&subject)?;
    let page = Page::new(source.path, route, kind, title, source.read.document);
    Some(page.dated(date, updated).tagged(tags).summarized(summary))
}

/// Every file left out, with the decision that left it out.
fn excluded(scope: &Scope<'_>, record: &Record) -> Vec<Excluded> {
    let outside = scope.outside.iter().map(|entry| Excluded::new(entry.path.clone(), DecisionCode::Root));
    let inside = scope.inside.iter().filter_map(|file| {
        let subject = file.subject();
        let because = if record.get::<code::Ignored>(&subject) == Some(&true) {
            DecisionCode::Ignored
        } else if record.get::<code::Publish>(&subject) == Some(&false) {
            DecisionCode::Publish
        } else {
            return None;
        };
        Some(Excluded::new(file.path().clone(), because))
    });
    outside.chain(inside).collect()
}

/// Site refusals the inferrers should have caught before.
fn diagnostic(error: SiteError, home: Option<PageId>) -> Diagnostic {
    match error {
        SiteError::Collision { route, files } => {
            Diagnostic::new(Problem::PageRouteCollision { route, files: files.into() }).on(Subject::Site)
        }
        SiteError::Missing { page } => {
            let decision = if home == Some(page) { DecisionCode::Home } else { DecisionCode::Nav };
            Broken::Missing { decision, subject: Subject::Site }.into()
        }
        other => Broken::Site { source: other }.into(),
    }
}
