//! A [`Decided`] written as the TOML `read/value.rs` reads back as it.

use jiff::civil;
use jiff::tz;
use model::{ContentPath, Decided, Root, When};
use toml_edit::{Array, Date, Datetime, Offset, Time, Value};

use super::Unwritable;
use crate::value::{KINDS, LAYOUTS, name_of};

/// The TOML that `read` reads back as `decided`.
pub(super) fn write(decided: &Decided) -> Result<Value, Unwritable> {
    let unknown = || Unwritable::Unknown { code: decided.code() };
    Ok(match decided {
        Decided::Root(Root::Top) => "".into(),
        Decided::Root(Root::Folder(path)) | Decided::Home(Some(path)) => path.as_str().into(),
        Decided::Layout(layout) => name_of(LAYOUTS, layout).ok_or_else(unknown)?.into(),
        Decided::Kind(kind) => name_of(KINDS, kind).ok_or_else(unknown)?.into(),
        Decided::Ignored(yes) | Decided::Publish(yes) => (*yes).into(),
        Decided::Home(None)
        | Decided::Date(None)
        | Decided::Updated(None)
        | Decided::Summary(None)
        | Decided::Description(None) => false.into(),
        Decided::Title(text)
        | Decided::SiteTitle(text)
        | Decided::Summary(Some(text))
        | Decided::Description(Some(text)) => text.as_str().into(),
        Decided::Date(Some(at)) | Decided::Updated(Some(at)) => {
            datetime(*at).ok_or(Unwritable::When { code: decided.code(), at: *at })?.into()
        }
        Decided::Slug(slug) => slug.as_str().into(),
        Decided::Tags(tags) => tags.iter().map(|tag| tag.written.as_str()).collect::<Array>().into(),
        Decided::Nav(paths) => paths.iter().map(ContentPath::as_str).collect::<Array>().into(),
        Decided::Authors(authors) => authors.iter().map(AsRef::<str>::as_ref).collect::<Array>().into(),
        Decided::Language(language) => language.as_ref().into(),
        // a variant added to `model` after this match: `Unwritable::Unknown`, and the
        // `every_code` test fails until it is written
        _ => return Err(unknown()),
    })
}

/// A day as a local date, an instant as a date and time in UTC. TOML writes years 0 to 9999.
fn datetime(at: When) -> Option<Datetime> {
    let date = |day: civil::Date| {
        Some(Date {
            year: u16::try_from(day.year()).ok().filter(|year| *year <= 9999)?,
            month: u8::try_from(day.month()).ok()?,
            day: u8::try_from(day.day()).ok()?,
        })
    };
    Some(match at {
        When::Day(day) => Datetime { date: Some(date(day)?), time: None, offset: None },
        When::Instant(instant) => {
            let utc = tz::Offset::UTC.to_datetime(instant);
            let nanosecond = u32::try_from(utc.subsec_nanosecond()).ok()?;
            Datetime {
                date: Some(date(utc.date())?),
                time: Some(Time {
                    hour: u8::try_from(utc.hour()).ok()?,
                    minute: u8::try_from(utc.minute()).ok()?,
                    second: Some(u8::try_from(utc.second()).ok()?),
                    nanosecond: (nanosecond != 0).then_some(nanosecond),
                }),
                offset: Some(Offset::Z),
            }
        }
        _ => return None,
    })
}
