//! What a key of `ink.toml` takes: the `expected` of `config.value.invalid`, written once.

use std::fmt;

use model::{ContentPath, DecisionCode, PathError};

use crate::{PatternError, ThemeIdError};

/// The shape a key's value must have, and why a value did not fit when that is known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Shape<'a> {
    /// The value of a pin.
    Pin(DecisionCode),
    /// `[site]`: a table of sitewide pins.
    Table(&'a str),
    /// `[page."x"]`: a table of one file's pins.
    FileTable { table: &'a str, file: &'a ContentPath },
    /// The key of a file's table, which is the file's path.
    FileKey(PathError),
    /// `include` or `exclude`, with the pattern that did not parse.
    Patterns(Option<PatternError>),
    /// `[theme]`.
    Theme,
    /// `use` in `[theme]`, with the id that did not parse.
    ThemeId(Option<ThemeIdError>),
    /// A token in `[theme]`.
    Token,
}

impl fmt::Display for Shape<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        const WHEN: &str = "a date (2024-03-01), a date and time with an offset (2024-03-01T09:30:00Z)";
        match self {
            Self::Pin(code) => match code {
                DecisionCode::Root => write!(out, "\"\" for the top of the source, or the path of a folder"),
                DecisionCode::Layout => write!(out, "one of \"blog\", \"vault\", \"docs\", \"flat\""),
                DecisionCode::Ignored | DecisionCode::Publish => write!(out, "true or false"),
                DecisionCode::Home => write!(out, "the path of a file, or false for a generated listing"),
                DecisionCode::Kind => write!(out, "one of \"home\", \"post\", \"page\""),
                DecisionCode::Title | DecisionCode::SiteTitle => write!(out, "a string"),
                DecisionCode::Date => write!(out, "{WHEN}, or false for undated"),
                DecisionCode::Updated => write!(out, "{WHEN}, or false for never updated"),
                DecisionCode::Slug => write!(out, "a slug: lower-case letters and digits, with single `-` inside"),
                DecisionCode::Tags => write!(out, "an array of strings, each with a letter or digit"),
                DecisionCode::Nav => write!(out, "an array of file paths"),
                DecisionCode::Summary | DecisionCode::Description => write!(out, "a string, or false for none"),
                DecisionCode::Authors => {
                    write!(out, "an array of names, each with a character that is not a space, such as [\"Ann\"]")
                }
                DecisionCode::Language => write!(
                    out,
                    "a language tag: two or three letters, then `-` and subtags of one to eight letters or digits, such as \"fr-CH\""
                ),
                // a code added to `model` after this match; the `every_code` test fails until it
                // has its own wording
                _ => write!(out, "a value this version of uses.ink does not read"),
            },
            Self::Table(table) => write!(out, "a table: `[{table}]`"),
            Self::FileTable { table, file } => {
                write!(out, "a table of pins: `[{}]`", crate::place::KeyPath(vec![table, file.as_str()]))
            }
            Self::FileKey(error) => write!(out, "the path of a file as its key, and {error}"),
            Self::Patterns(None) => write!(out, "an array of glob patterns (`*`, `?`, `**`)"),
            Self::Patterns(Some(error)) => write!(out, "an array of glob patterns (`*`, `?`, `**`), and {error}"),
            Self::Theme => write!(out, "a table: `[theme]` with `use = \"<scope>:theme-<variant>\"` and tokens"),
            Self::ThemeId(None) => write!(out, "a theme plugin id, such as \"ink:theme-quiet\""),
            Self::ThemeId(Some(error)) => write!(out, "a theme plugin id, such as \"ink:theme-quiet\", and {error}"),
            Self::Token => write!(out, "a string"),
        }
    }
}
