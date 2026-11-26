//! `page.publish`: whether a page is part of the site.

use model::{ContentPath, DefaultReason, Evidence};

use crate::interpret::file::read::Source;
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::record::Record;
use crate::pins::Pins;

/// A folder of unfinished pages.
const DRAFTS: &str = "drafts";

/// Whether pages are published until said otherwise, or only once selected (decision 0017).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Selection {
    /// A public source: every page is published unless it says otherwise.
    Implicit,
    /// A vault or a private source: only selected pages are published.
    Explicit,
}

/// Pin, then frontmatter `publish`, then frontmatter `draft`, then an `include` pattern (a
/// vault's selection), then a draft's name (a `drafts` folder or a leading `_`), then the
/// default: published, except in a vault or a private source, where nothing is published
/// until it is selected (decision 0017).
pub(crate) fn decide(source: &Source, selection: Selection, pins: &impl Pins, record: &mut Record) -> bool {
    let front = &source.read.front;
    let (default, reason) = match selection {
        Selection::Implicit => (true, DefaultReason::NothingMatched),
        Selection::Explicit => (false, DefaultReason::NotSelected),
    };
    Ladder::<code::Publish>::new(source.subject(), pins, record)
        .rung(front.publish.as_ref().map(|publish| (publish.value, source.frontmatter(publish.span))))
        .rung(front.draft.as_ref().map(|draft| (!draft.value, source.frontmatter(draft.span))))
        .rung(pins.included(&source.path).map(|span| (true, Evidence::Config { span })))
        .rung(is_draft(&source.relative).then(|| (false, source.filename())))
        .or_default(default, reason, record)
}

fn is_draft(relative: &ContentPath) -> bool {
    relative.segments().any(|segment| segment == DRAFTS) || relative.file_name().starts_with('_')
}
