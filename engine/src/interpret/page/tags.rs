//! `page.tags`.

use model::DefaultReason;

use crate::interpret::file::read::Source;
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::record::Record;
use crate::pins::Pins;

/// Pin, then frontmatter `tags`, then none.
pub(crate) fn decide(source: &Source, pins: &impl Pins, record: &mut Record) {
    let front = &source.read.front;
    Ladder::<code::Tags>::new(source.subject(), pins, record)
        .rung(front.tags.as_ref().map(|tags| (tags.value.clone(), source.frontmatter(tags.span))))
        .or_default(Vec::new(), DefaultReason::NothingMatched, record);
}
