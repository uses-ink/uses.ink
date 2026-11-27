//! The stage deciding everything about each file a format read, in order: `page.publish`,
//! `page.kind`, `page.title`, `page.date`, `page.updated`, `page.slug`, `page.tags`,
//! `page.summary`.

use std::collections::BTreeSet;

use model::code::{Code, Date};
use model::decision::Decision;
use model::{Announcement, ContentPath, DefaultReason, Evidence, FileFacts, Known, Notice, PageKind};

use crate::Pins;
use crate::interpret::decide::ladder::About;
use crate::interpret::decide::{Explain, Filed, Sourced, precedence};
use crate::interpret::file::read::Source;
use crate::interpret::journal::Journal;
use crate::interpret::stage::Stage;

/// Whether pages are published until said otherwise, or only once selected (decision 0017).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Selection {
    /// A public source: every page is published unless it says otherwise.
    Implicit,
    /// A vault or a private source: only selected pages are published.
    Explicit,
}

/// What a page's decisions are made on: the file read, and how its site selects pages.
#[derive(Debug)]
pub(crate) struct Ground<'a> {
    pub(crate) source: &'a Source,
    pub(crate) selection: Selection,
}

impl Filed for Ground<'_> {
    fn path(&self) -> &ContentPath {
        &self.source.path
    }

    fn relative(&self) -> &ContentPath {
        &self.source.relative
    }

    fn facts(&self) -> Option<&FileFacts> {
        self.source.facts.as_ref()
    }
}

impl Sourced for Ground<'_> {
    fn source(&self) -> Option<&Source> {
        Some(self.source)
    }
}

/// Decides everything about each page.
#[derive(Debug)]
pub(crate) struct Pages<'a, P> {
    pub(crate) pins: &'a P,
    pub(crate) selection: Selection,
}

impl<P: Pins> Stage<&[Source]> for Pages<'_, P> {
    /// The pages published.
    type Out = BTreeSet<ContentPath>;

    fn run(&self, sources: &[Source], journal: &mut Journal) -> BTreeSet<ContentPath> {
        let mut published = BTreeSet::new();
        for source in sources {
            if self.page(source, journal) == Some(true) {
                published.insert(source.path.clone());
            }
        }
        published
    }
}

impl<P: Pins> Pages<'_, P> {
    /// Decides one page. Gives back whether it is published.
    fn page(&self, source: &Source, journal: &mut Journal) -> Option<bool> {
        let ground = Ground { source, selection: self.selection };
        let subject = source.subject();
        let about = About { subject: &subject, ground: &ground };
        let published = precedence::Publish::default().record(about, self.pins, journal);
        let kind = precedence::Kind::default().record(about, self.pins, journal);
        precedence::Title::default().record(about, self.pins, journal);
        precedence::Date::default().record(about, self.pins, journal);
        let undated =
            journal.decision::<Date>(&subject).and_then(|date| Undated::announce(date, source.facts.as_ref()));
        if let Some(announcement) = undated.filter(|_| kind == Some(PageKind::Post)) {
            journal.notice(Notice::new(announcement).on(subject.clone().into()));
        }
        precedence::Updated::default().record(about, self.pins, journal);
        precedence::Slug::default().record(about, self.pins, journal);
        precedence::Tags::default().record(about, self.pins, journal);
        precedence::Summary::default().record(about, self.pins, journal);
        published
    }
}

/// A post nothing dates, said: the feeds leave it out.
#[derive(Debug)]
struct Undated;

impl Undated {
    /// `fact.date.bound` when the history only bounds the date, the bound read from the file's
    /// `facts`, else `notice.default.chosen`; nothing when `decision` dates the page or was
    /// pinned.
    fn announce(decision: &Decision<Date>, facts: Option<&FileFacts>) -> Option<Announcement> {
        let Evidence::Default { reason } = decision.evidence else { return None };
        if decision.value.is_some() {
            return None;
        }
        let bound = facts.map(|facts| &facts.created).filter(|_| reason == DefaultReason::OnlyBound);
        Some(match bound {
            Some(Known::NotAfter { value, .. }) => {
                Announcement::FactDateBound { decision: Date::CODE, bound: model::When::Instant(*value).to_string() }
            }
            _ => Date::chosen(&None),
        })
    }
}
