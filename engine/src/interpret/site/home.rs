//! `site.home`: the page served at `/`.

use std::collections::BTreeSet;

use model::{ContentPath, DecisionCode, DefaultReason, Diagnostic, Problem, Subject};

use crate::interpret::file::read::Source;
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::record::Record;
use crate::pins::Pins;

/// Stems of a home page at the top of the root, in precedence order.
const HOMES: &[&str] = &["index", "README", "readme", "Home", "Index"];

/// Pin, then the first published page at the top of the root named as in [`HOMES`], then a
/// generated listing. A pinned home that is not a published page is `pin.target.unpublished`.
pub(crate) fn decide(
    sources: &[Source],
    published: &BTreeSet<ContentPath>,
    pins: &impl Pins,
    record: &mut Record,
) -> Option<ContentPath> {
    let mut ladder = Ladder::<code::Home>::new(Subject::Site, pins, record);
    for stem in HOMES {
        let found = sources.iter().filter(|source| {
            source.relative.parent().is_none() && source.relative.stem() == *stem && published.contains(&source.path)
        });
        for source in found {
            ladder = ladder.rung(Some((Some(source.path.clone()), source.filename())));
        }
    }
    let home = ladder.or_default(None, DefaultReason::NothingMatched, record);
    if let Some(path) = &home
        && !published.contains(path)
    {
        let problem = Problem::PinTargetUnpublished { decision: DecisionCode::Home, path: path.clone() };
        record.diagnose(Diagnostic::new(problem).on(Subject::Site));
    }
    home
}
