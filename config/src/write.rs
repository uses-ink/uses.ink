//! Writing decisions as pins: `ink explain --pin` and the CLI's pin commands. The only module
//! that sees `toml_edit`. Values are written exactly as [`Config::parse`](crate::Config::parse)
//! reads them, so the file read back pins every decision it was written from.

mod written;

use std::error::Error;
use std::fmt;

use model::decision::{Decision, Scope};
use model::{AnyCode, AnyDecision, Code, Span, Subject, Typed, When};
use toml_edit::{DocumentMut, InlineTable, Item, TableLike, TomlError, Value};

pub(crate) use written::Written;

use crate::Offsets;
use crate::pin::{KeyPath, Place};
use crate::value::Toml;

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

impl Document {
    /// See [`crate::pins`].
    pub(crate) fn pins<'a>(decisions: impl IntoIterator<Item = &'a AnyDecision>) -> String {
        let mut document = Self(DocumentMut::new());
        let unwritten = NotPinned(document.put(decisions).into_iter().filter_map(Result::err).collect());
        if !unwritten.0.is_empty() {
            document.0.set_trailing(format!("\n{unwritten}"));
        }
        document.0.to_string()
    }

    /// See [`crate::pins_into`].
    pub(crate) fn pins_into<'a>(
        existing: &str,
        decisions: impl IntoIterator<Item = &'a AnyDecision>,
    ) -> Result<String, PinsError> {
        let mut document = Self(existing.parse().map_err(|error| PinsError::Syntax(SyntaxError(error)))?);
        document.put(decisions).into_iter().collect::<Result<(), _>>()?;
        Ok(document.0.to_string())
    }
}

/// The document being written.
#[derive(Debug)]
pub(crate) struct Document(DocumentMut);

impl Document {
    /// Pins each decision, in file order (sitewide first, then by table, by file and by the
    /// code's place in decision order), and says how each went.
    fn put<'a>(&mut self, decisions: impl IntoIterator<Item = &'a AnyDecision>) -> Vec<Result<(), PinsError>> {
        let mut ordered: Vec<(Place, Subject, &AnyDecision)> = decisions
            .into_iter()
            .map(|decision| (Place::from(decision.code()), decision.subject(), decision))
            .collect();
        ordered.sort_by(|(a, a_subject, _), (b, b_subject, _)| {
            let key = |place: &Place, subject| (place.code.scope() == Scope::File, place.table, subject, place.code);
            key(a, a_subject).cmp(&key(b, b_subject))
        });
        ordered.into_iter().map(|(_, _, decision)| decision.each(&mut Put(self))).collect()
    }

    /// Pins `decision` at its place.
    fn pin<C: Code<Value: Toml>>(&mut self, decision: &Decision<C>) -> Result<(), PinsError> {
        let place = Place::from(C::CODE);
        let subject: Subject = decision.subject.clone().into();
        let value = match decision.value.write() {
            Ok(Written(value)) => value,
            Err(source) => return Err(PinsError::Value { code: C::CODE, subject, source }),
        };
        let holds = match C::CODE.scope() {
            Scope::Site => Holds::Pins,
            Scope::File => Holds::Tables,
        };
        let root = Table { inner: self.0.as_table_mut(), form: Form::Header };
        let mut target = root.open(place.table, holds);
        if let Subject::File(path) = &subject {
            target = target.and_then(|table| table.open(path.as_str(), Holds::Pins));
        }
        match target {
            Some(table) => {
                table.set(place.key, value);
                Ok(())
            }
            None => Err(PinsError::NotTable { code: C::CODE, subject }),
        }
    }
}

/// The document a decision is pinned into, as its code's type.
#[derive(Debug)]
struct Put<'a>(&'a mut Document);

impl<C: Code<Value: Toml>> Typed<C> for Put<'_> {
    type Out = Result<(), PinsError>;

    fn each(&mut self, decision: &Decision<C>) -> Self::Out {
        self.0.pin(decision)
    }
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

/// A table of the document, and how it is written.
struct Table<'a> {
    inner: &'a mut dyn TableLike,
    form: Form,
}

impl Table<'_> {
    /// The table at `name`, made in this table's form if missing. `None` when `name` holds
    /// something other than a table.
    fn open(self, name: &str, holds: Holds) -> Option<Self> {
        let form = self.form;
        let item = self.inner.entry(name).or_insert_with(|| match form {
            Form::Inline => Item::Value(Value::InlineTable(InlineTable::new())),
            Form::Header => {
                let mut table = toml_edit::Table::new();
                table.set_implicit(holds == Holds::Tables);
                Item::Table(table)
            }
        });
        let form = if item.is_inline_table() { Form::Inline } else { Form::Header };
        item.as_table_like_mut().map(|inner| Table { inner, form })
    }

    /// Sets `key`, keeping the key and the comments and spacing around a value already there.
    fn set(self, key: &str, mut value: Value) {
        match self.inner.get_mut(key) {
            Some(item) => {
                if let Some(old) = item.as_value() {
                    *value.decor_mut() = old.decor().clone();
                }
                *item = Item::Value(value);
            }
            None => {
                self.inner.insert(key, Item::Value(value));
            }
        }
    }
}

/// The comment [`pins`] ends with when some decisions could not be written: each error and its
/// causes on a line.
struct NotPinned(Vec<PinsError>);

impl fmt::Display for NotPinned {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for error in &self.0 {
            write!(out, "# not pinned: {error}")?;
            let mut cause = error.source();
            while let Some(source) = cause {
                write!(out, ": {source}")?;
                cause = source.source();
            }
            writeln!(out)?;
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
        self.0.span().map(|range| Offsets(range).into())
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "ink.toml does not parse")
    }
}

impl Error for SyntaxError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

/// A value TOML cannot hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unwritable {
    /// A date outside the years TOML writes, 0 to 9999.
    Year(When),
    /// A value the model added after this version of uses.ink, which it has no spelling for.
    Unknown,
}

impl fmt::Display for Unwritable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Year(at) => write!(out, "{at} is outside the years TOML writes, 0 to 9999"),
            Self::Unknown => write!(out, "this version of uses.ink has no spelling for the value"),
        }
    }
}

impl Error for Unwritable {}

/// Why a decision could not be pinned.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PinsError {
    /// The existing file does not parse.
    Syntax(SyntaxError),
    /// A key on the way to the pin holds something other than a table, as `site = 3` does.
    NotTable {
        /// The code being pinned.
        code: AnyCode,
        /// What it is pinned on.
        subject: Subject,
    },
    /// The decided value cannot be written in TOML.
    Value {
        /// The code being pinned.
        code: AnyCode,
        /// What it is pinned on.
        subject: Subject,
        /// Why the value cannot be written.
        source: Unwritable,
    },
}

impl fmt::Display for PinsError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(_) => write!(out, "pins cannot be written into ink.toml"),
            Self::NotTable { code, subject } => {
                write!(out, "`{code}` cannot be pinned on {subject}: a key on its way in ink.toml is not a table")
            }
            Self::Value { code, subject, .. } => write!(out, "`{code}` on {subject} cannot be written as a pin"),
        }
    }
}

impl Error for PinsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Syntax(error) => Some(error),
            Self::Value { source, .. } => Some(source),
            Self::NotTable { .. } => None,
        }
    }
}
