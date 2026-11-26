//! The stage deciding what is decided once for the site, after every page: `site.home`,
//! `site.title`, `site.description`, `site.authors`, `site.language`, `site.nav`. Then routes,
//! and the site itself.

mod assemble;
mod route;

use std::collections::BTreeSet;

use model::code::{Code, Home, Nav};
use model::decision::Site as Whole;
use model::{ContentPath, Diagnostic, Problem, Subject};

pub(crate) use assemble::{Assemble, Parts};
pub(crate) use route::Routing;

use crate::Pins;
use crate::interpret::decide::ladder::{Case, Ladder, Testimony, Witness};
use crate::interpret::decide::{Explain, Filed, Sourced, precedence};
use crate::interpret::file::read::Source;
use crate::interpret::journal::Journal;
use crate::interpret::stage::Stage;
use crate::interpret::tree::Scope;

/// What the site's decisions are made on: every page, which are published, the home once it is
/// decided, and the decisions so far.
#[derive(Debug)]
pub(crate) struct Ground<'a> {
    pub(crate) scope: &'a Scope<'a>,
    pub(crate) sources: &'a [Source],
    pub(crate) published: &'a BTreeSet<ContentPath>,
    pub(crate) home: Option<&'a Source>,
    pub(crate) journal: &'a Journal,
}

impl Sourced for Ground<'_> {
    /// The home page.
    fn source(&self) -> Option<&Source> {
        self.home
    }
}

/// A published page at the top of the root named as a home, evidence `Filename`, in the
/// precedence of [`Index::STEMS`].
#[derive(Debug, Default)]
pub(crate) struct Index;

impl Index {
    /// Stems of a home page, in precedence order.
    const STEMS: &[&str] = &["index", "README", "readme", "Home", "Index"];
}

impl Witness<Home, Ground<'_>> for Index {
    fn testify(&self, case: &Case<'_, Home, Ground<'_>>, testimony: &mut Testimony<Option<ContentPath>>) {
        let ground = case.ground;
        for stem in Self::STEMS {
            let found = ground.sources.iter().filter(|source| {
                source.relative.parent().is_none()
                    && source.relative.stem() == *stem
                    && ground.published.contains(&source.path)
            });
            for source in found {
                testimony.value(Some(source.path.clone()), source.named());
            }
        }
    }
}

/// Decides everything about the site as a whole.
#[derive(Debug)]
pub(crate) struct Site<'a, P> {
    pub(crate) pins: &'a P,
    pub(crate) scope: &'a Scope<'a>,
    pub(crate) published: &'a BTreeSet<ContentPath>,
}

impl<P: Pins> Stage<&[Source]> for Site<'_, P> {
    /// The home page's file, or `None` for a generated listing.
    type Out = Option<ContentPath>;

    /// A pinned home or navigation entry that is not a published page is
    /// `pin.target.unpublished`.
    fn run(&self, sources: &[Source], journal: &mut Journal) -> Option<ContentPath> {
        let home = self.decide(&precedence::Home::default(), sources, None, journal).flatten();
        self.unpublished::<Home>(home.iter(), journal);
        let page = sources.iter().find(|source| Some(&source.path) == home.as_ref());
        self.decide(&precedence::SiteTitle::default(), sources, page, journal);
        self.decide(&precedence::Description::default(), sources, page, journal);
        self.decide(&precedence::Authors::default(), sources, page, journal);
        self.decide(&precedence::Language::default(), sources, page, journal);
        let nav = self.decide(&precedence::Nav::default(), sources, page, journal);
        self.unpublished::<Nav>(nav.iter().flatten(), journal);
        home
    }
}

impl<P: Pins> Site<'_, P> {
    /// Decides `C` for the site and records it. The ground holds the journal, so the decision is
    /// recorded once the ground is gone.
    fn decide<C: Explain<Subject = Whole>, W: for<'g> Witness<C, Ground<'g>>>(
        &self,
        ladder: &Ladder<C, W>,
        sources: &[Source],
        home: Option<&Source>,
        journal: &mut Journal,
    ) -> Option<C::Value> {
        let pin = self.pins.pin::<C>(&Whole);
        let decided = {
            let ground = Ground { scope: self.scope, sources, published: self.published, home, journal };
            ladder.decide(&Case { subject: &Whole, pin, patterns: self.pins, ground: &ground })
        };
        journal.record(decided)
    }

    /// Says each of `paths` that `C` names and that is not a published page.
    fn unpublished<'p, C: Code>(&self, paths: impl IntoIterator<Item = &'p ContentPath>, journal: &mut Journal) {
        for path in paths.into_iter().filter(|path| !self.published.contains(*path)) {
            let problem = Problem::PinTargetUnpublished { decision: C::CODE, path: path.clone() };
            journal.diagnose(Diagnostic::new(problem).on(Subject::Site));
        }
    }
}
