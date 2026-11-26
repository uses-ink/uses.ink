//! `page.title`.

use model::Evidence;

use crate::interpret::file::read::Source;
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::record::Record;
use crate::pins::Pins;

/// Pin, then frontmatter `title`, then the first level-1 heading, then the file name.
pub(crate) fn decide(source: &Source, pins: &impl Pins, record: &mut Record) {
    let front = &source.read.front;
    let heading =
        source.read.heading.as_ref().map(|heading| {
            (heading.value.clone(), Evidence::Heading { path: source.path.clone(), span: heading.span })
        });
    Ladder::<code::Title>::new(source.subject(), pins, record)
        .rung(front.title.as_ref().map(|title| (title.value.clone(), source.frontmatter(title.span))))
        .rung(heading)
        .or(source.name().title(), source.filename(), record);
}
