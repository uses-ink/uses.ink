//! The decisions made once per file a format read, in order: publish, kind, title, date and
//! updated, slug, tags, summary.

mod date;
pub(crate) mod kind;
mod publish;
mod slug;
mod summary;
mod tags;
mod title;

pub(crate) use publish::Selection;

use crate::interpret::file::read::Source;
use crate::interpret::record::Record;
use crate::pins::Pins;

/// Decides everything about one page. Returns whether it is published.
pub(crate) fn decide(source: &Source, selection: Selection, pins: &impl Pins, record: &mut Record) -> bool {
    let published = publish::decide(source, selection, pins, record);
    let kind = kind::decide(source, pins, record);
    title::decide(source, pins, record);
    date::decide(source, kind, pins, record);
    slug::decide(source, pins, record);
    tags::decide(source, pins, record);
    summary::decide(source, pins, record);
    published
}
