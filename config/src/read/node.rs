//! One value of `ink.toml` as toml parsed it, with its span: what [`Toml::read`] reads, so no
//! value type sees the `toml` crate.
//!
//! [`Toml::read`]: crate::value::Toml::read

use jiff::{civil, tz};
use model::{Span, When};
use toml::de::{DeString, DeTable, DeValue};
use toml::value::{Date, Datetime, Offset};

use crate::Offsets;

/// One value, and where it is written.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Node<'a>(&'a toml::Spanned<DeValue<'a>>);

/// A key as written, and where.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Key<'a> {
    /// The key, its quotes and escapes resolved.
    pub(crate) name: &'a str,
    /// Where it is.
    pub(crate) span: Span,
}

/// A table's entries, in the order written.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Table<'a>(pub(super) &'a DeTable<'a>);

/// Every wrong part of one value, never empty: the value itself, or each wrong element of an
/// array.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Refused {
    first: Span,
    rest: Vec<Span>,
}

impl<'a> From<&'a toml::Spanned<DeValue<'a>>> for Node<'a> {
    fn from(value: &'a toml::Spanned<DeValue<'a>>) -> Self {
        Self(value)
    }
}

impl<'a> From<&'a toml::Spanned<DeString<'a>>> for Key<'a> {
    fn from(key: &'a toml::Spanned<DeString<'a>>) -> Self {
        Self { name: key.get_ref().as_ref(), span: Offsets(key.span()).into() }
    }
}

impl<'a> Table<'a> {
    /// Each key and its value, in the order written.
    pub(crate) fn entries(self) -> impl Iterator<Item = (Key<'a>, Node<'a>)> {
        self.0.iter().map(|(key, value)| (Key::from(key), Node::from(value)))
    }
}

impl<'a> Node<'a> {
    /// Where it is.
    pub(crate) fn span(self) -> Span {
        Offsets(self.0.span()).into()
    }

    /// The refusal of the whole value.
    pub(crate) fn refused(self) -> Refused {
        Refused { first: self.span(), rest: Vec::new() }
    }

    /// The string it is.
    pub(crate) fn text(self) -> Result<&'a str, Refused> {
        self.0.get_ref().as_str().ok_or_else(|| self.refused())
    }

    /// The boolean it is.
    pub(crate) fn flag(self) -> Result<bool, Refused> {
        self.0.get_ref().as_bool().ok_or_else(|| self.refused())
    }

    /// Whether it is `false`, which says "none".
    pub(crate) fn is_false(self) -> bool {
        matches!(self.0.get_ref(), DeValue::Boolean(false))
    }

    /// The elements of the array it is.
    pub(crate) fn items(self) -> Result<impl Iterator<Item = Self>, Refused> {
        Ok(self.0.get_ref().as_array().ok_or_else(|| self.refused())?.iter().map(Self))
    }

    /// The table it is, inline or not.
    pub(crate) fn table(self) -> Option<Table<'a>> {
        self.0.get_ref().as_table().map(Table)
    }

    /// The [`When`] it is: a local date is a day; a date and time with an offset is an instant.
    /// A local date and time names no instant without a zone the author never gave, so it is
    /// refused, as is a time alone.
    pub(crate) fn when(self) -> Result<When, Refused> {
        let DeValue::Datetime(at) = self.0.get_ref() else { return Err(self.refused()) };
        Stamp(at).when().ok_or_else(|| self.refused())
    }
}

/// A TOML date and time, read as a [`When`].
#[derive(Clone, Copy, Debug)]
struct Stamp<'a>(&'a Datetime);

impl Stamp<'_> {
    fn when(self) -> Option<When> {
        match (self.0.date, self.0.time, self.0.offset) {
            (Some(date), None, None) => Self::day(date).map(When::Day),
            (Some(date), Some(time), Some(offset)) => {
                let time = civil::Time::new(
                    i8::try_from(time.hour).ok()?,
                    i8::try_from(time.minute).ok()?,
                    i8::try_from(time.second.unwrap_or(0)).ok()?,
                    i32::try_from(time.nanosecond.unwrap_or(0)).ok()?,
                )
                .ok()?;
                let seconds = match offset {
                    Offset::Z => 0,
                    Offset::Custom { minutes } => i32::from(minutes) * 60,
                };
                let offset = tz::Offset::from_seconds(seconds).ok()?;
                offset.to_timestamp(Self::day(date)?.to_datetime(time)).ok().map(When::Instant)
            }
            _ => None,
        }
    }

    fn day(date: Date) -> Option<civil::Date> {
        civil::Date::new(i16::try_from(date.year).ok()?, i8::try_from(date.month).ok()?, i8::try_from(date.day).ok()?)
            .ok()
    }
}

impl Refused {
    /// Every value of `results`, or every refused part among them.
    pub(crate) fn gather<T>(results: impl IntoIterator<Item = Result<T, Self>>) -> Result<Vec<T>, Self> {
        let mut values = Vec::new();
        let mut refused: Option<Self> = None;
        for result in results {
            match (result, &mut refused) {
                (Ok(value), _) => values.push(value),
                (Err(more), Some(all)) => all.rest.extend(more),
                (Err(more), None) => refused = Some(more),
            }
        }
        refused.map_or(Ok(values), Err)
    }
}

impl IntoIterator for Refused {
    type Item = Span;
    type IntoIter = std::iter::Chain<std::iter::Once<Span>, std::vec::IntoIter<Span>>;

    fn into_iter(self) -> Self::IntoIter {
        std::iter::once(self.first).chain(self.rest)
    }
}
