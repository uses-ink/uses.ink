//! Each code's own words: how its value reads in `ink explain` and notices (an unset date is
//! `undated`, an unset update `never updated`), and which defaults are announced. The words are
//! the code's, not its value type's: `page.publish` and `file.ignore` are both a `bool`.

use std::fmt;
use std::marker::PhantomData;

use model::code::{
    Authors, Code, Date, Description, Home, Ignored, Kind, Language, Layout, Nav, Publish, Root, SiteTitle, Slug,
    Summary, Tags, Title, Updated,
};
use model::decision::Decision;
use model::{Announcement, DefaultReason, Evidence, Value};

/// A code that can say what its values mean and what taking its default means.
pub(crate) trait Explain: Code {
    /// Writes `value` as a person reads it for this code. By default, as the value type shows
    /// itself.
    fn word(value: &Self::Value, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        value.show(out)
    }

    /// The announcement taking `value` by default, for `reason`, calls for. Defaults that only
    /// mean "nothing to do" (nothing ignored, no tags, never updated, a public page published)
    /// announce nothing, or every file would; this is the default.
    fn announce(_reason: DefaultReason, _value: &Self::Value) -> Option<Announcement> {
        None
    }

    /// The announcement `decision` calls for, when it took a default.
    fn notice(decision: &Decision<Self>) -> Option<Announcement> {
        let Evidence::Default { reason } = decision.evidence else { return None };
        Self::announce(reason, &decision.value)
    }

    /// `notice.default.chosen` for `value`.
    fn chosen(value: &Self::Value) -> Announcement {
        Announcement::NoticeDefaultChosen { decision: Self::CODE.into(), value: Worded::<Self>::new(value).to_string() }
    }
}

/// A value of code `C`, in `C`'s words.
pub(crate) struct Worded<'a, C: Code> {
    value: &'a C::Value,
    code: PhantomData<C>,
}

impl<'a, C: Code> Worded<'a, C> {
    /// `value`, worded as `C` words it.
    pub(crate) const fn new(value: &'a C::Value) -> Self {
        Self { value, code: PhantomData }
    }
}

impl<C: Explain> fmt::Display for Worded<'_, C> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        C::word(self.value, out)
    }
}

/// Implements [`Explain`] for codes whose default is always announced and whose words are
/// their value type's. The same impl over a list of types, which a generic cannot write.
///
/// ```text
/// announced!(Root, Layout);
/// // makes `impl Explain for Root` announcing `notice.default.chosen` on every default
/// ```
macro_rules! announced {
    ($($code:ty),+) => {
        $(
            impl Explain for $code {
                fn announce(_reason: DefaultReason, value: &Self::Value) -> Option<Announcement> {
                    Some(Self::chosen(value))
                }
            }
        )+
    };
}

announced!(Root, Layout, Kind, Slug, SiteTitle, Language);

impl Explain for Title {}

impl Explain for Summary {}

impl Explain for Description {}

impl Explain for Ignored {
    /// `ignored` or `kept`.
    fn word(value: &bool, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(if *value { "ignored" } else { "kept" })
    }
}

impl Explain for Publish {
    /// `published` or `not published`.
    fn word(value: &bool, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(if *value { "published" } else { "not published" })
    }

    /// Only a vault's or a private source's default is worth saying: nothing is published until
    /// it is selected.
    fn announce(reason: DefaultReason, value: &bool) -> Option<Announcement> {
        (reason == DefaultReason::NotSelected).then(|| Self::chosen(value))
    }
}

impl Explain for Date {
    /// The date, or `undated`. An undated post is announced by the page stage, which knows it is
    /// a post.
    fn word(value: &Option<model::When>, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match value {
            Some(when) => when.show(out),
            None => out.write_str("undated"),
        }
    }
}

impl Explain for Updated {
    /// The date, or `never updated`.
    fn word(value: &Option<model::When>, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match value {
            Some(when) => when.show(out),
            None => out.write_str("never updated"),
        }
    }
}

impl Explain for Tags {
    /// The tags, or `no tags`.
    fn word(value: &Vec<model::Tag>, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        if value.is_empty() { out.write_str("no tags") } else { value.show(out) }
    }
}

impl Explain for Home {
    /// The home's file, or `a generated listing`.
    fn word(value: &Option<model::ContentPath>, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match value {
            Some(path) => fmt::Display::fmt(path, out),
            None => out.write_str("a generated listing"),
        }
    }

    /// A generated home is always said.
    fn announce(_reason: DefaultReason, _value: &Option<model::ContentPath>) -> Option<Announcement> {
        Some(Announcement::SiteHomeGenerated)
    }
}

impl Explain for Nav {
    /// The files joined by `, `, or `empty`.
    fn word(value: &Vec<model::ContentPath>, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some((first, rest)) = value.split_first() else { return out.write_str("empty") };
        fmt::Display::fmt(first, out)?;
        rest.iter().try_for_each(|path| write!(out, ", {path}"))
    }

    fn announce(_reason: DefaultReason, value: &Vec<model::ContentPath>) -> Option<Announcement> {
        Some(Self::chosen(value))
    }
}

impl Explain for Authors {
    /// The names, or `no one`.
    fn word(value: &Vec<model::Author>, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        if value.is_empty() { out.write_str("no one") } else { value.show(out) }
    }

    fn announce(_reason: DefaultReason, value: &Vec<model::Author>) -> Option<Announcement> {
        Some(Self::chosen(value))
    }
}
