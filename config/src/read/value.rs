//! A pin's value read from TOML into a [`Decided`]; `write/value.rs` is the other way.

use std::ops::Range;

use jiff::civil;
use jiff::tz;
use model::{Author, ContentPath, Decided, DecisionCode, Language, Root, Slug, Tag, When};
use toml::Spanned;
use toml::de::DeValue;
use toml::value::{Date, Datetime, Offset};

use crate::value::{KINDS, LAYOUTS, named};

/// Reads the value pinned for `code`, or the range of the part of it that is wrong: the
/// whole value, or the one element of an array.
pub(super) fn read(code: DecisionCode, value: &Spanned<DeValue<'_>>) -> Result<Decided, Range<usize>> {
    let whole = value.span();
    let value = value.get_ref();
    let decided = match code {
        DecisionCode::Root => match value.as_str() {
            Some("") => Some(Decided::Root(Root::Top)),
            Some(text) => ContentPath::parse(text).ok().map(|path| Decided::Root(Root::Folder(path))),
            None => None,
        },
        DecisionCode::Layout => value.as_str().and_then(|text| named(LAYOUTS, text)).map(Decided::Layout),
        DecisionCode::Ignored => value.as_bool().map(Decided::Ignored),
        DecisionCode::Publish => value.as_bool().map(Decided::Publish),
        DecisionCode::Home => match value {
            DeValue::Boolean(false) => Some(Decided::Home(None)),
            DeValue::String(text) => ContentPath::parse(text.as_ref()).ok().map(|path| Decided::Home(Some(path))),
            _ => None,
        },
        DecisionCode::Kind => value.as_str().and_then(|text| named(KINDS, text)).map(Decided::Kind),
        DecisionCode::Title => value.as_str().map(|text| Decided::Title(text.to_owned())),
        DecisionCode::Date => undated(value, Decided::Date),
        DecisionCode::Updated => undated(value, Decided::Updated),
        DecisionCode::Slug => value.as_str().and_then(|text| Slug::parse(text).ok()).map(Decided::Slug),
        DecisionCode::Tags => return each(value, whole, |text| Tag::new(text).ok()).map(Decided::Tags),
        DecisionCode::Nav => return each(value, whole, |text| ContentPath::parse(text).ok()).map(Decided::Nav),
        DecisionCode::Summary => text_or_none(value, Decided::Summary),
        DecisionCode::SiteTitle => value.as_str().map(|text| Decided::SiteTitle(text.to_owned())),
        DecisionCode::Description => text_or_none(value, Decided::Description),
        DecisionCode::Authors => {
            return each(value, whole, |text| Author::try_from(text.to_owned()).ok()).map(Decided::Authors);
        }
        DecisionCode::Language => value.as_str().and_then(|text| text.parse::<Language>().ok()).map(Decided::Language),
        // a code added to `model` after this match: `Shape` says it is not read, and the
        // `every_code` test fails until it is
        _ => None,
    };
    decided.ok_or(whole)
}

/// An array of strings, each read by `one`; the range is of the first element that fails.
fn each<T>(value: &DeValue<'_>, whole: Range<usize>, one: impl Fn(&str) -> Option<T>) -> Result<Vec<T>, Range<usize>> {
    let array = value.as_array().ok_or(whole)?;
    array.iter().map(|element| element.get_ref().as_str().and_then(&one).ok_or_else(|| element.span())).collect()
}

/// A string, or `false` for none, as `decided`.
fn text_or_none(value: &DeValue<'_>, decided: fn(Option<String>) -> Decided) -> Option<Decided> {
    match value {
        DeValue::Boolean(false) => Some(decided(None)),
        DeValue::String(text) => Some(decided(Some(text.as_ref().to_owned()))),
        _ => None,
    }
}

/// A [`When`], or `false` for none, as `decided`.
fn undated(value: &DeValue<'_>, decided: fn(Option<When>) -> Decided) -> Option<Decided> {
    match value {
        DeValue::Boolean(false) => Some(decided(None)),
        DeValue::Datetime(at) => when(at).map(|at| decided(Some(at))),
        _ => None,
    }
}

/// A local date is a day; a date and time with an offset is an instant. A local date and time
/// names no instant without a zone the author never gave, so it is refused.
fn when(at: &Datetime) -> Option<When> {
    match (at.date, at.time, at.offset) {
        (Some(date), None, None) => day(date).map(When::Day),
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
            offset.to_timestamp(day(date)?.to_datetime(time)).ok().map(When::Instant)
        }
        _ => None,
    }
}

fn day(date: Date) -> Option<civil::Date> {
    civil::Date::new(i16::try_from(date.year).ok()?, i8::try_from(date.month).ok()?, i8::try_from(date.day).ok()?).ok()
}
