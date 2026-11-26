//! `page.date` and `page.updated`. History facts count only when exact: a `Known::NotAfter`
//! bound is recorded as a rejected alternative, and an undated post says why.

use model::{Announcement, DecisionCode, DefaultReason, Evidence, Known, Notice, PageKind, Timestamp, When};

use crate::interpret::file::read::Source;
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::record::Record;
use crate::pins::Pins;

type Rung = Option<(Option<When>, Evidence)>;

/// `page.date`: pin, then frontmatter `date`, then a dated file name, then the history's exact
/// creation, then undated. `page.updated`: pin, then frontmatter `updated`, then the history's
/// exact last change, then never updated.
pub(crate) fn decide(source: &Source, kind: PageKind, pins: &impl Pins, record: &mut Record) {
    let front = &source.read.front;
    let facts = source.facts.as_ref();
    let created = facts.map(|facts| &facts.created);
    let created_bound = bound(created);
    let date = Ladder::<code::Date>::new(source.subject(), pins, record)
        .rung(front.date.as_ref().map(|date| (Some(date.value), source.frontmatter(date.span))))
        .rung(source.name().date().map(|day| (Some(When::Day(day)), source.filename())))
        .rung(exact(created))
        .bound(created_bound.clone())
        .or_default(None, DefaultReason::NothingMatched, record);
    let defaulted = record
        .decision::<code::Date>(&source.subject())
        .is_some_and(|decision| matches!(decision.evidence, Evidence::Default { .. }));
    if kind == PageKind::Post && date.is_none() && defaulted {
        let announcement = match created_bound {
            Some((Some(bound), _)) => {
                Announcement::FactDateBound { decision: DecisionCode::Date, bound: bound.to_string() }
            }
            _ => Announcement::NoticeDefaultChosen { decision: DecisionCode::Date, value: "undated".to_owned() },
        };
        record.notice(Notice::new(announcement).on(source.subject()));
    }
    let updated = facts.map(|facts| &facts.updated);
    Ladder::<code::Updated>::new(source.subject(), pins, record)
        .rung(front.updated.as_ref().map(|updated| (Some(updated.value), source.frontmatter(updated.span))))
        .rung(exact(updated))
        .bound(bound(updated))
        .or_default(None, DefaultReason::NothingMatched, record);
}

/// The history's value, when it is exact.
fn exact(known: Option<&Known<Timestamp>>) -> Rung {
    match known? {
        Known::Exact { value, from } => Some((Some(When::Instant(*value)), Evidence::History { from: from.clone() })),
        _ => None,
    }
}

/// The history's bound, when a bound is all it knows.
fn bound(known: Option<&Known<Timestamp>>) -> Rung {
    match known? {
        Known::NotAfter { value, from } => {
            Some((Some(When::Instant(*value)), Evidence::History { from: from.clone() }))
        }
        _ => None,
    }
}
