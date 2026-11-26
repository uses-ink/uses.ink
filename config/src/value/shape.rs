//! What a key of `ink.toml` takes, worded once: the `expected` of `config.value.invalid`. A pin's
//! shape comes from its value type ([`Toml::SHAPE`]); `Option<T>` and `Vec<T>` build theirs from
//! `T`'s, and the code pinned says what `false` means ([`Shape::pin`]).

use std::fmt;

use model::{ContentPath, PathError, Word};

use super::Toml;
use crate::pin::KeyPath;
use crate::{PatternError, ThemeIdError};

/// The shape a value must have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shape<'a> {
    /// Any string.
    Text,
    /// `true` or `false`.
    Flag,
    /// A date, or a date and time with an offset.
    When,
    /// A slug.
    Slug,
    /// A language tag.
    Language,
    /// A tag: a string with a letter or digit.
    Tag,
    /// An author's name.
    Author,
    /// The path of a file.
    Path,
    /// `""` or the path of a folder.
    Root,
    /// One of these names.
    One(&'a [&'a str]),
    /// The shape, or `false` for none, worded as the pinned code words it: `undated` for
    /// `page.date` ([`Shape::pin`]).
    Optional {
        /// What a value is.
        shape: &'a Shape<'a>,
        /// What `false` means.
        none: &'a str,
    },
    /// An array, each element of the shape.
    Array(&'a Shape<'a>),
    /// A glob pattern.
    Pattern,
    /// `[site]`: a table of pins.
    Table(&'a str),
    /// `[page."x"]`: a table of one file's pins.
    Pins {
        /// `page`.
        table: &'a str,
        /// `x`.
        file: &'a ContentPath,
    },
    /// The key of a file's table, which is the file's path.
    FileKey,
    /// `[theme]`.
    Theme,
    /// `use` in `[theme]`.
    ThemeId,
}

impl Shape<'static> {
    /// What a pin of code `C` takes: its value type's shape, with `false` worded as `C` words
    /// none ([`Word::NONE`]).
    pub(crate) const fn pin<C: Word<Value: Toml>>() -> Self {
        match C::Value::SHAPE {
            Self::Optional { shape, .. } => Self::Optional { shape, none: C::NONE },
            shape => shape,
        }
    }
}

/// The values of `when` in a sentence.
const WHEN: &str = "a date (2024-03-01), a date and time with an offset (2024-03-01T09:30:00Z)";

impl fmt::Display for Shape<'_> {
    /// One value: `a string`.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text => out.write_str("a string"),
            Self::Flag => out.write_str("true or false"),
            Self::When => out.write_str(WHEN),
            Self::Slug => out.write_str("a slug: lower-case letters and digits, with single `-` inside"),
            Self::Language => out.write_str(
                "a language tag: two or three letters, then `-` and subtags of one to eight letters or digits, such as \"fr-CH\"",
            ),
            Self::Tag => out.write_str("a string with a letter or digit"),
            Self::Author => out.write_str("a name with a character that is not a space, such as \"Ann\""),
            Self::Path => out.write_str("the path of a file"),
            Self::Root => out.write_str("\"\" for the top of the source, or the path of a folder"),
            Self::One(names) => {
                out.write_str("one of ")?;
                for (index, name) in names.iter().enumerate() {
                    let comma = if index == 0 { "" } else { ", " };
                    write!(out, "{comma}\"{name}\"")?;
                }
                Ok(())
            }
            Self::Optional { shape, none } => write!(out, "{shape}, or false for {none}"),
            Self::Array(shape) => write!(out, "an array of {}", Plural(**shape)),
            Self::Pattern => out.write_str("a glob pattern (`*`, `?`, `**`)"),
            Self::Table(table) => write!(out, "a table: `[{table}]`"),
            Self::Pins { table, file } => write!(out, "a table of pins: `[{}]`", KeyPath(vec![table, file.as_str()])),
            Self::FileKey => out.write_str("the path of a file as its key"),
            Self::Theme => out.write_str("a table: `[theme]` with `use = \"<scope>:theme-<variant>\"` and tokens"),
            Self::ThemeId => out.write_str("a theme plugin id, such as \"ink:theme-quiet\""),
        }
    }
}

/// A shape as the elements of an array: `strings`.
#[derive(Clone, Copy, Debug)]
struct Plural<'a>(Shape<'a>);

impl fmt::Display for Plural<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Shape::Text => out.write_str("strings"),
            Shape::Tag => out.write_str("strings, each with a letter or digit"),
            Shape::Author => out.write_str("names, each with a character that is not a space, such as [\"Ann\"]"),
            Shape::Path => out.write_str("file paths"),
            Shape::Pattern => out.write_str("glob patterns (`*`, `?`, `**`)"),
            Shape::When => out.write_str("dates"),
            Shape::Slug => out.write_str("slugs"),
            Shape::Language => out.write_str("language tags"),
            shape => write!(out, "values, each {shape}"),
        }
    }
}

/// What a key wanted, and why the value did not fit when the value type says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Expected<'a> {
    shape: Shape<'a>,
    cause: Option<Cause>,
}

impl<'a> From<Shape<'a>> for Expected<'a> {
    fn from(shape: Shape<'a>) -> Self {
        Self { shape, cause: None }
    }
}

impl<'a> Expected<'a> {
    /// `shape`, refused for `cause`.
    pub(crate) fn because(shape: Shape<'a>, cause: impl Into<Cause>) -> Self {
        Self { shape, cause: Some(cause.into()) }
    }
}

impl fmt::Display for Expected<'_> {
    /// `the shape`, or `the shape, and the cause`.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            Some(cause) => write!(out, "{}, and {cause}", self.shape),
            None => write!(out, "{}", self.shape),
        }
    }
}

/// Why a value type refused a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Cause {
    /// Not a path in the snapshot.
    Path(PathError),
    /// Not a glob pattern.
    Pattern(PatternError),
    /// Not a theme plugin id.
    Theme(ThemeIdError),
}

impl fmt::Display for Cause {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Path(error) => fmt::Display::fmt(error, out),
            Self::Pattern(error) => fmt::Display::fmt(error, out),
            Self::Theme(error) => fmt::Display::fmt(error, out),
        }
    }
}

impl From<PathError> for Cause {
    fn from(error: PathError) -> Self {
        Self::Path(error)
    }
}

impl From<PatternError> for Cause {
    fn from(error: PatternError) -> Self {
        Self::Pattern(error)
    }
}

impl From<ThemeIdError> for Cause {
    fn from(error: ThemeIdError) -> Self {
        Self::Theme(error)
    }
}
