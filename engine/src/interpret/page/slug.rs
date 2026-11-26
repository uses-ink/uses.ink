//! `page.slug`: the last part of a page's route.

use model::{DefaultReason, PageId, Slug};

use crate::interpret::file::read::Source;
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::record::Record;
use crate::pins::Pins;

/// Pin, then frontmatter `slug`, then the file name after its date, then the page id's first
/// digits for a name with no letter or digit.
pub(crate) fn decide(source: &Source, pins: &impl Pins, record: &mut Record) {
    let front = &source.read.front;
    let named = Slug::from_text(source.name().rest()).ok().map(|slug| (slug, source.filename()));
    Ladder::<code::Slug>::new(source.subject(), pins, record)
        .rung(front.slug.as_ref().map(|slug| (slug.value.clone(), source.frontmatter(slug.span))))
        .rung(named)
        .or_default(PageId::of(&source.path).slug(), DefaultReason::NothingMatched, record);
}
