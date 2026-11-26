//! Each code's own words: how its value reads to a person in `ink explain`, notices and
//! `ink.toml`'s errors (an unset date is `undated`, an unset update `never updated`). The words
//! are the code's, not its value type's: `page.publish` and `file.ignore` are both a `bool`, and
//! `page.date` and `page.updated` both an `Option<When>`.

use std::fmt;
use std::marker::PhantomData;

use super::{
    Authors, Code, Date, Description, Home, Ignored, Kind, Language, Layout, Nav, Publish, Root, SiteTitle, Slug,
    Summary, Tags, Title, Updated,
};
use crate::{ContentPath, Value};

/// A code that says what its values mean. Every code of the closed set implements it, so a
/// generic consumer bounds `C: Word` and never words a value itself.
pub trait Word: Code {
    /// What the code's empty value means: none, or an empty list. `page.date = false` pins
    /// `undated`; config says so when a pin has the wrong shape.
    const NONE: &'static str = "none";

    /// Writes `value` as a person reads it for this code: [`Word::NONE`] for an empty value,
    /// else as the value type shows itself.
    fn word(value: &Self::Value, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        if value.is_empty() { out.write_str(Self::NONE) } else { value.show(out) }
    }
}

/// A value of code `C`, in `C`'s words.
pub struct Worded<'a, C: Code> {
    value: &'a C::Value,
    code: PhantomData<C>,
}

impl<'a, C: Code> Worded<'a, C> {
    /// `value`, worded as `C` words it.
    pub const fn new(value: &'a C::Value) -> Self {
        Self { value, code: PhantomData }
    }
}

impl<C: Code> fmt::Debug for Worded<'_, C> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_tuple("Worded").field(self.value).finish()
    }
}

impl<C: Word> fmt::Display for Worded<'_, C> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        C::word(self.value, out)
    }
}

impl Word for Root {}

impl Word for Layout {}

impl Word for Kind {}

impl Word for Title {}

impl Word for Summary {}

impl Word for Slug {}

impl Word for SiteTitle {}

impl Word for Description {}

impl Word for Language {}

impl Word for Ignored {
    /// `ignored` or `kept`.
    fn word(value: &bool, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(if *value { "ignored" } else { "kept" })
    }
}

impl Word for Publish {
    /// `published` or `not published`.
    fn word(value: &bool, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(if *value { "published" } else { "not published" })
    }
}

impl Word for Date {
    const NONE: &'static str = "undated";
}

impl Word for Updated {
    const NONE: &'static str = "never updated";
}

impl Word for Tags {
    const NONE: &'static str = "no tags";
}

impl Word for Authors {
    const NONE: &'static str = "no one";
}

impl Word for Home {
    const NONE: &'static str = "a generated listing";

    /// The home's file, or its `NONE`.
    fn word(value: &Option<ContentPath>, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match value {
            Some(path) => fmt::Display::fmt(path, out),
            None => out.write_str(Self::NONE),
        }
    }
}

impl Word for Nav {
    const NONE: &'static str = "empty";

    /// The files joined by `, `, or its `NONE`.
    fn word(value: &Vec<ContentPath>, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some((first, rest)) = value.split_first() else { return out.write_str(Self::NONE) };
        fmt::Display::fmt(first, out)?;
        rest.iter().try_for_each(|path| write!(out, ", {path}"))
    }
}
