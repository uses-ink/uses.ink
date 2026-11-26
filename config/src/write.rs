//! Writing decisions as pins: `ink explain --pin` and the CLI's pin commands. The only module
//! that sees `toml_edit`. The values are written exactly as [`Config::parse`](crate::Config::parse)
//! reads them, so the file read back pins every decision it was written from.

use std::fmt;

use model::{Decision, DecisionCode, Subject, When};
use toml_edit::{DocumentMut, InlineTable, Item, Table, TableLike, TomlError, Value};

use crate::place::{self, KeyPath, Reach};
use model::Span;

mod value;

impl fmt::Display for KeyPath<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, segment) in self.0.iter().enumerate() {
            if index > 0 {
                out.write_str(".")?;
            }
            out.write_str(&toml_edit::Key::new(*segment).display_repr())?;
        }
        Ok(())
    }
}

/// An existing `ink.toml` that does not parse, keeping the parser's error as its source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxError(TomlError);

impl SyntaxError {
    /// Where the parser stopped, if it says.
    pub fn span(&self) -> Option<Span> {
        self.0.span().map(place::span)
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "ink.toml does not parse")
    }
}

impl std::error::Error for SyntaxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// A decision that cannot be written as a pin.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unwritable {
    /// A sitewide code on a file, or a per-file code on the site: no key pins it.
    Subject {
        /// The decision's code.
        code: DecisionCode,
        /// What it was decided on.
        subject: Subject,
    },
    /// A date outside what TOML writes, years 0 to 9999.
    When {
        /// The decision's code.
        code: DecisionCode,
        /// The date.
        at: When,
    },
    /// A code or a value this version of uses.ink cannot write.
    Unknown {
        /// The decision's code.
        code: DecisionCode,
    },
}

impl fmt::Display for Unwritable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Subject { code, subject } => write!(out, "`{code}` is not pinned on {subject}"),
            Self::When { code, at } => write!(out, "`{code}` is {at}; TOML writes years 0 to 9999"),
            Self::Unknown { code } => write!(out, "`{code}` cannot be written as a pin yet"),
        }
    }
}

impl std::error::Error for Unwritable {}

/// Why pins could not be written into an existing `ink.toml`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PinsError {
    /// The existing file does not parse.
    Syntax(SyntaxError),
    /// A key on the way to the pin holds something other than a table, as `site = 3` does.
    NotTable {
        /// The code being pinned.
        code: DecisionCode,
        /// What it is pinned on.
        subject: Subject,
    },
    /// The decision has no pin.
    Unwritable(Unwritable),
}

impl fmt::Display for PinsError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(_) => write!(out, "pins cannot be written into ink.toml"),
            Self::NotTable { code, subject } => {
                write!(out, "`{code}` cannot be pinned on {subject}: a key on its way in ink.toml is not a table")
            }
            Self::Unwritable(_) => write!(out, "a decision cannot be written as a pin"),
        }
    }
}

impl std::error::Error for PinsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Syntax(error) => Some(error),
            Self::Unwritable(unwritable) => Some(unwritable),
            Self::NotTable { .. } => None,
        }
    }
}

/// The comment `pins` ends with when some decisions could not be written.
struct NotPinned(Vec<PinsError>);

impl fmt::Display for NotPinned {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for error in &self.0 {
            match error {
                PinsError::Unwritable(unwritable) => writeln!(out, "# not pinned: {unwritable}")?,
                other => writeln!(out, "# not pinned: {other}")?,
            }
        }
        Ok(())
    }
}

impl From<Unwritable> for PinsError {
    fn from(unwritable: Unwritable) -> Self {
        Self::Unwritable(unwritable)
    }
}

/// A whole `ink.toml` that pins every decision: sitewide tables first, then the per-file tables
/// (`[file."..."]`, then `[page."..."]`), each sorted by path, keys in decision order. A later
/// decision with the same code and subject replaces an earlier one. A decision that cannot be
/// written is listed in a comment at the end, never dropped.
pub fn pins(decisions: &[Decision]) -> String {
    let mut ordered: Vec<&Decision> = decisions.iter().collect();
    ordered.sort_by_key(|decision| order(decision));
    let mut document = DocumentMut::new();
    let unwritten = NotPinned(ordered.into_iter().filter_map(|decision| put(&mut document, decision).err()).collect());
    if !unwritten.0.is_empty() {
        document.set_trailing(format!("\n{unwritten}"));
    }
    document.to_string()
}

/// `existing` with every decision pinned in it: values already there are replaced in place,
/// keeping their comments; new tables go after the existing ones.
pub fn pins_into(existing: &str, decisions: &[Decision]) -> Result<String, PinsError> {
    let mut document: DocumentMut = existing.parse().map_err(|error| PinsError::Syntax(SyntaxError(error)))?;
    let mut ordered: Vec<&Decision> = decisions.iter().collect();
    ordered.sort_by_key(|decision| order(decision));
    for decision in ordered {
        put(&mut document, decision)?;
    }
    Ok(document.to_string())
}

/// Sitewide first, then by table, by file and by the code's place in the inferrer order.
fn order(decision: &Decision) -> (bool, &'static str, Option<&str>, usize) {
    let code = decision.code();
    let file = match &decision.subject {
        Subject::File(path) => Some(path.as_str()),
        _ => None,
    };
    let table = place::of(code).map_or("", |(table, _)| table);
    let rank = DecisionCode::ALL.iter().position(|c| *c == code).unwrap_or(usize::MAX);
    (!code.is_sitewide(), table, file, rank)
}

fn put(document: &mut DocumentMut, decision: &Decision) -> Result<(), PinsError> {
    let code = decision.code();
    let (table, key) = place::of(code).ok_or(Unwritable::Unknown { code })?;
    let file = match &decision.subject {
        Subject::Site if code.is_sitewide() => None,
        Subject::File(path) if !code.is_sitewide() => Some(path.as_str()),
        subject => return Err(Unwritable::Subject { code, subject: subject.clone() }.into()),
    };
    let value = value::write(&decision.decided)?;
    let not_table = || PinsError::NotTable { code, subject: decision.subject.clone() };
    let holds = match Reach::from(code) {
        Reach::Site => Holds::Pins,
        Reach::File => Holds::Tables,
    };
    let (mut target, form) = open(document.as_table_mut(), Form::Header, table, holds).ok_or_else(not_table)?;
    if let Some(file) = file {
        (target, _) = open(target, form, file, Holds::Pins).ok_or_else(not_table)?;
    }
    set(target, key, value);
    Ok(())
}

/// How a table is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Form {
    /// Under a `[header]` or as dotted keys.
    Header,
    /// `{ ... }`.
    Inline,
}

/// What a table holds, which decides whether a new one gets a header of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Holds {
    /// Keys: `[site]` is written.
    Pins,
    /// Only tables: `[page]` is implicit, never written on its own line.
    Tables,
}

/// The table at `name` in `parent` (written in `form`), made if missing, and its own form.
/// `None` when `name` holds something other than a table.
fn open<'a>(
    parent: &'a mut dyn TableLike,
    form: Form,
    name: &str,
    holds: Holds,
) -> Option<(&'a mut dyn TableLike, Form)> {
    let item = parent.entry(name).or_insert_with(|| match form {
        Form::Inline => Item::Value(Value::InlineTable(InlineTable::new())),
        Form::Header => {
            let mut table = Table::new();
            table.set_implicit(holds == Holds::Tables);
            Item::Table(table)
        }
    });
    let form = if item.is_inline_table() { Form::Inline } else { Form::Header };
    item.as_table_like_mut().map(|table| (table, form))
}

/// Sets `key`, keeping the key and the comments and spacing around a value already there.
fn set(table: &mut dyn TableLike, key: &str, mut value: Value) {
    match table.get_mut(key) {
        Some(item) => {
            if let Some(old) = item.as_value() {
                *value.decor_mut() = old.decor().clone();
            }
            *item = Item::Value(value);
        }
        None => {
            table.insert(key, Item::Value(value));
        }
    }
}
