//! One value as it is written into `ink.toml`: what [`Toml::write`] gives, so no value type sees
//! `toml_edit`.
//!
//! [`Toml::write`]: crate::value::Toml::write

use jiff::{civil, tz};
use model::When;
use toml_edit::{Array, Date, Datetime, Offset, Time, Value};

use super::{LAST_YEAR, Unwritable};

/// A TOML value.
#[derive(Clone, Debug)]
pub(crate) struct Written(pub(super) Value);

impl From<&str> for Written {
    fn from(text: &str) -> Self {
        Self(text.into())
    }
}

impl From<bool> for Written {
    fn from(flag: bool) -> Self {
        Self(flag.into())
    }
}

impl FromIterator<Written> for Written {
    /// An array of the values.
    fn from_iter<I: IntoIterator<Item = Written>>(values: I) -> Self {
        Self(values.into_iter().map(|Written(value)| value).collect::<Array>().into())
    }
}

impl TryFrom<When> for Written {
    type Error = Unwritable;

    /// A day as a local date, an instant as a date and time in UTC. TOML writes years 0 to
    /// [`LAST_YEAR`].
    fn try_from(at: When) -> Result<Self, Self::Error> {
        let year = || Unwritable::Year(at);
        let date = |day: civil::Date| {
            Some(Date {
                year: u16::try_from(day.year()).ok().filter(|year| *year <= LAST_YEAR)?,
                month: u8::try_from(day.month()).ok()?,
                day: u8::try_from(day.day()).ok()?,
            })
        };
        let datetime = match at {
            When::Day(day) => Datetime { date: Some(date(day).ok_or_else(year)?), time: None, offset: None },
            When::Instant(instant) => {
                let utc = tz::Offset::UTC.to_datetime(instant);
                let time = || {
                    let nanosecond = u32::try_from(utc.subsec_nanosecond()).ok()?;
                    Some(Time {
                        hour: u8::try_from(utc.hour()).ok()?,
                        minute: u8::try_from(utc.minute()).ok()?,
                        second: Some(u8::try_from(utc.second()).ok()?),
                        nanosecond: (nanosecond != 0).then_some(nanosecond),
                    })
                };
                Datetime {
                    date: Some(date(utc.date()).ok_or_else(year)?),
                    time: Some(time().ok_or(Unwritable::Unknown)?),
                    offset: Some(Offset::Z),
                }
            }
            _ => return Err(Unwritable::Unknown),
        };
        Ok(Self(datetime.into()))
    }
}
