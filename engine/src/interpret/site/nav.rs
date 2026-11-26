//! `site.nav`: the pages the navigation lists, in order.

use std::collections::BTreeSet;

use model::{ContentPath, DecisionCode, DefaultReason, Diagnostic, PageKind, Problem, Subject};

use crate::interpret::file::read::Source;
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::record::Record;
use crate::pins::Pins;

/// What the navigation is chosen from.
#[derive(Debug)]
pub(crate) struct Choice<'a> {
    pub(crate) sources: &'a [Source],
    pub(crate) published: &'a BTreeSet<ContentPath>,
    pub(crate) home: Option<&'a ContentPath>,
}

/// Pin, then every published page at the top of the root that is neither a post nor the home,
/// by title then path. A pinned entry that is not a published page is `pin.target.unpublished`.
pub(crate) fn decide(choice: &Choice<'_>, pins: &impl Pins, record: &mut Record) {
    let mut listed: Vec<(Option<String>, ContentPath)> = choice
        .sources
        .iter()
        .filter(|source| source.relative.parent().is_none() && choice.published.contains(&source.path))
        .filter(|source| Some(&source.path) != choice.home)
        .filter(|source| record.get::<code::Kind>(&source.subject()) != Some(&PageKind::Post))
        .map(|source| (record.get::<code::Title>(&source.subject()).cloned(), source.path.clone()))
        .collect();
    listed.sort();
    let default = listed.into_iter().map(|(_, path)| path).collect();
    let nav = Ladder::<code::Nav>::new(Subject::Site, pins, record).or_default(
        default,
        DefaultReason::NothingMatched,
        record,
    );
    for path in nav.into_iter().filter(|path| !choice.published.contains(path)) {
        let problem = Problem::PinTargetUnpublished { decision: DecisionCode::Nav, path };
        record.diagnose(Diagnostic::new(problem).on(Subject::Site));
    }
}
