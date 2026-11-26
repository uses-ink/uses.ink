//! Where a pin lives in `ink.toml`, derived from its decision code's name: `site.home` is key
//! `home` of table `site`; a per-file code adds the file's path between the two.

use std::ops::Range;

use model::{DecisionCode, Span};

/// The table and key of a code: `page.title` is (`page`, `title`). Every name is `table.key`
/// (conventions.md, Grammar); one that is not has no place and is never read or written.
pub(crate) fn of(code: DecisionCode) -> Option<(&'static str, &'static str)> {
    code.name().split_once('.')
}

/// The code pinned at `key` of `table`, if any.
pub(crate) fn code_at(table: &str, key: &str) -> Option<DecisionCode> {
    DecisionCode::ALL.iter().copied().find(|code| of(*code) == Some((table, key)))
}

/// What a code is decided for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reach {
    /// Once for the site: `[site] home`.
    Site,
    /// Once per file: `[page."x"] title`.
    File,
}

impl From<DecisionCode> for Reach {
    fn from(code: DecisionCode) -> Self {
        if code.is_sitewide() { Self::Site } else { Self::File }
    }
}

/// Whether `table` holds pins of codes with this reach.
pub(crate) fn holds(table: &str, reach: Reach) -> bool {
    DecisionCode::ALL.iter().any(|code| Reach::from(*code) == reach && of(*code).is_some_and(|(name, _)| name == table))
}

/// A dotted key as TOML writes it: `page."posts/x.md".title`. Its `Display` is in `write`,
/// which owns `toml_edit`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeyPath<'a>(pub(crate) Vec<&'a str>);

/// An empty span at the start, for a range that cannot be one (toml never gives such a range).
const START: Span = match Span::new(0, 0) {
    Ok(span) => span,
    // evaluated at compile time, and 0 is not after 0
    Err(_) => panic!("an empty span"),
};

/// A byte range from toml as a [`Span`].
pub(crate) fn span(range: Range<usize>) -> Span {
    Span::new(range.start, range.end).unwrap_or(START)
}

/// From the start of `first` to the end of `last`: a whole `key = value`.
pub(crate) fn across(first: Range<usize>, last: Range<usize>) -> Span {
    span(first.start..last.end.max(first.start))
}
