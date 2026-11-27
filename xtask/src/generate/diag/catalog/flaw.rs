//! Everything that can be wrong with the catalog.

use std::fmt;

use super::{Fault, Key, Kind, Named, Part, Severity};

/// One thing wrong with the catalog.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Flaw {
    /// A top-level key other than `diagnostic`, `notice` and `field`, or one that is not a table.
    Section {
        name: String,
    },
    /// A code that is not `noun.noun.reason` in kebab case.
    Code {
        code: String,
    },
    /// Two codes that make the same Rust variant.
    Collision {
        code: String,
        other: String,
    },
    /// An entry that is not a table.
    NotTable {
        code: String,
    },
    /// A key the entry may not have.
    Key {
        code: String,
        key: String,
    },
    /// A key the entry needs.
    Missing {
        code: String,
        key: Key,
    },
    /// A key whose value is not a string, or not a table for `fields`.
    NotText {
        code: String,
        key: Key,
    },
    Severity {
        code: String,
        given: String,
    },
    Fault {
        code: String,
        given: String,
    },
    /// A field name that is not a lower-case Rust identifier.
    FieldName {
        code: String,
        field: String,
    },
    /// A field type outside the closed set.
    FieldType {
        code: String,
        field: String,
        given: String,
    },
    /// A doc in `[field]` that is not a string.
    FieldDoc {
        field: String,
    },
    /// A field whose name has no doc in `[field]`.
    Undocumented {
        code: String,
        field: String,
    },
    /// A doc in `[field]` for a name no entry has.
    Unused {
        field: String,
    },
    /// A template that uses a field the entry does not declare.
    Undeclared {
        code: String,
        part: Part,
        field: String,
    },
    /// A `{` or `}` that opens or closes nothing, at this byte offset of the template.
    Brace {
        code: String,
        part: Part,
        at: usize,
    },
}

impl fmt::Display for Flaw {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Section { name } => {
                write!(out, "`{name}` is not a section; the sections are `diagnostic`, `notice` and `field`")
            }
            Self::Code { code } => write!(out, "`{code}` is not a code: write `noun.noun.reason` in kebab case"),
            Self::Collision { code, other } => write!(out, "`{code}` and `{other}` make the same Rust name"),
            Self::NotTable { code } => write!(out, "`{code}` is not a table"),
            Self::Key { code, key } => write!(out, "`{code}` has `{key}`, which it may not have"),
            Self::Missing { code, key } => write!(out, "`{code}` has no `{key}`"),
            Self::NotText { code, key } => write!(out, "`{code}`: `{key}` is not a string"),
            Self::Severity { code, given } => {
                write!(out, "`{code}`: `{given}` is not a severity; write {}", Either::<Severity>::SET)
            }
            Self::Fault { code, given } => {
                write!(out, "`{code}`: `{given}` is not a fault; write {}", Either::<Fault>::SET)
            }
            Self::FieldName { code, field } => {
                write!(out, "`{code}`: `{field}` is not a field name; write a lower-case Rust identifier")
            }
            Self::FieldType { code, field, given } => {
                let known: Vec<&str> = Kind::NAMES.iter().map(|(name, _)| *name).collect();
                write!(out, "`{code}`: field `{field}` has type `{given}`; the types are {}", known.join(", "))
            }
            Self::FieldDoc { field } => write!(out, "`field.{field}` is not a string"),
            Self::Undocumented { code, field } => {
                write!(out, "`{code}`: field `{field}` has no doc; write one under `[field]`")
            }
            Self::Unused { field } => write!(out, "`field.{field}` documents a field no entry has"),
            Self::Undeclared { code, part, field } => {
                write!(out, "`{code}`: the {} uses `{{{field}}}`, which is not a declared field", part.key())
            }
            Self::Brace { code, part, at } => {
                write!(out, "`{code}`: the {} has an unmatched brace at byte {at}; write `{{{{` or `}}}}`", part.key())
            }
        }
    }
}

/// The members of a [`Named`] set as a choice reads: `a, b or c`.
struct Either<T>(std::marker::PhantomData<T>);

impl<T> Either<T> {
    const SET: Self = Self(std::marker::PhantomData);
}

impl<T: Named> fmt::Display for Either<T> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let last = T::NAMES.len().saturating_sub(1);
        T::NAMES.iter().enumerate().try_for_each(|(i, (name, _))| {
            let sep = match i {
                0 => "",
                i if i == last => " or ",
                _ => ", ",
            };
            write!(out, "{sep}{name}")
        })
    }
}
