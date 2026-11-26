//! `page.kind`: whether a page is a post or a page.

use model::{DefaultReason, PageKind};

use crate::interpret::file::read::Source;
use crate::interpret::ladder::{Ladder, code};
use crate::interpret::record::Record;
use crate::pins::Pins;

/// Top folders whose pages are posts.
pub(crate) const POSTS: &[&str] = &["posts", "blog", "writing"];

/// Pin, then frontmatter `type`, then a top folder of [`POSTS`], then a frontmatter date, then
/// a dated file name, then a page.
pub(crate) fn decide(source: &Source, pins: &impl Pins, record: &mut Record) -> PageKind {
    let front = &source.read.front;
    let in_posts =
        source.relative.parent().is_some() && source.relative.segments().next().is_some_and(|top| POSTS.contains(&top));
    Ladder::<code::Kind>::new(source.subject(), pins, record)
        .rung(front.kind.as_ref().map(|kind| (kind.value, source.frontmatter(kind.span))))
        .rung(in_posts.then(|| (PageKind::Post, source.filename())))
        .rung(front.date.as_ref().map(|date| (PageKind::Post, source.frontmatter(date.span))))
        .rung(source.name().date().map(|_| (PageKind::Post, source.filename())))
        .or_default(PageKind::Page, DefaultReason::NothingMatched, record)
}
