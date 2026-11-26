//! The last stage: the site, built only from decisions. Each value is read back from the
//! journal, so nothing in the site lacks a cause.

use model::code::{
    AnyCode, Authors, Description, Home, Ignored, Kind, Language, Layout, Nav, Publish, SiteTitle, Summary, Tags,
    Title, Updated,
};
use model::decision::{File, Site as Whole};
use model::{Asset, Diagnostic, Excluded, Identity, Page, PageId, PageKind, Problem, SiteError, SiteParts, Subject};

use crate::error::Error;
use crate::interpret::file::read::Source;
use crate::interpret::journal::Journal;
use crate::interpret::site::route::Routes;
use crate::interpret::stage::Stage;
use crate::interpret::tree::{InRoot, Scope};

/// What the site is assembled from, besides the journal.
#[derive(Debug)]
pub(crate) struct Parts<'a> {
    pub(crate) scope: &'a Scope<'a>,
    /// Every file a format read, its links resolved.
    pub(crate) sources: Vec<Source>,
    /// The plain files published.
    pub(crate) assets: &'a [InRoot<'a>],
    pub(crate) routes: &'a Routes,
}

/// Builds the site, unless the journal holds an error: all or nothing (open.md Q6).
#[derive(Debug)]
pub(crate) struct Assemble;

impl Stage<Parts<'_>> for Assemble {
    type Out = Option<model::Site>;

    fn run(&self, parts: Parts<'_>, journal: &mut Journal) -> Option<model::Site> {
        if journal.stops() {
            return None;
        }
        let Parts { scope, sources, assets, routes } = parts;
        let home = journal.needed::<Home>(&Whole)?;
        let mut identity = Identity::new(journal.needed::<SiteTitle>(&Whole)?, journal.needed::<Language>(&Whole)?);
        identity.description = journal.needed::<Description>(&Whole)?;
        identity.authors = journal.needed::<Authors>(&Whole)?;
        let mut site = SiteParts::new(identity, scope.root.clone(), journal.needed::<Layout>(&Whole)?);
        site.home = home.as_ref().map(PageId::of);
        site.nav = journal.needed::<Nav>(&Whole)?.iter().map(PageId::of).collect();
        site.excluded = Self::excluded(scope, journal);
        for source in sources {
            if journal.value::<Publish>(&source.subject()) == Some(&true) {
                site.pages.push(Self::page(source, routes, home.as_ref(), journal)?);
            }
        }
        // A plain file without a route already said so (`route.segment.unnamed`).
        for file in assets {
            if let Some(route) = routes.assets.get(&file.entry.path) {
                site.assets.push(Asset::new(file.entry.path.clone(), route.clone(), file.entry.blob));
            }
        }
        let home = site.home;
        model::Site::new(site).map_err(|error| journal.diagnose(Self::refused(error, home))).ok()
    }
}

impl Assemble {
    fn page(source: Source, routes: &Routes, home: Option<&model::ContentPath>, journal: &mut Journal) -> Option<Page> {
        let subject = source.subject();
        let route = routes.pages.get(&source.path)?.clone();
        let kind = if Some(&source.path) == home { PageKind::Home } else { journal.needed::<Kind>(&subject)? };
        let title = journal.needed::<Title>(&subject)?;
        let date = journal.needed::<model::code::Date>(&subject)?;
        let updated = journal.needed::<Updated>(&subject)?;
        let tags = journal.needed::<Tags>(&subject)?;
        let summary = journal.needed::<Summary>(&subject)?;
        let page = Page::new(source.path, route, kind, title).holding(source.read.document);
        Some(page.dated(date, updated).tagged(tags).summarized(summary))
    }

    /// Every file left out, with the decision that left it out.
    fn excluded(scope: &Scope<'_>, journal: &Journal) -> Vec<Excluded> {
        let outside = scope.outside.iter().map(|entry| Excluded::new(entry.path.clone(), AnyCode::Root));
        let inside = scope.inside.iter().filter_map(|file| {
            let subject = File(file.entry.path.clone());
            let because = if journal.value::<Ignored>(&subject) == Some(&true) {
                AnyCode::Ignored
            } else if journal.value::<Publish>(&subject) == Some(&false) {
                AnyCode::Publish
            } else {
                return None;
            };
            Some(Excluded::new(subject.0, because))
        });
        outside.chain(inside).collect()
    }

    /// Site refusals the ladders should have caught before.
    fn refused(error: SiteError, home: Option<PageId>) -> Diagnostic {
        match error {
            SiteError::Collision { route, files } => {
                Diagnostic::new(Problem::PageRouteCollision { route, files: files.into() }).on(Subject::Site)
            }
            SiteError::Missing { page } => {
                let code = if home == Some(page) { AnyCode::Home } else { AnyCode::Nav };
                Error::Missing { code, subject: Subject::Site }.into()
            }
            other => Error::Site { source: other }.into(),
        }
    }
}
