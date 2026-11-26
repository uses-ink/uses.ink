//! A [`Problem`](crate::Problem) or an [`Announcement`](crate::Announcement) with its variant
//! erased: its code and its fields in catalog order, each field one of the catalog's eight field
//! types. This is the form that crosses the plugin boundary (`ink:model/diagnostic` in WIT), so a
//! new catalog entry changes no WIT. `cargo xtask diag` generates the conversions both ways.
//!
//! ```
//! use uses_ink_model::coded::{Coded, Field};
//! use uses_ink_model::Problem;
//!
//! let problem = Problem::HtmlElementStripped { element: "script".to_owned() };
//! let coded = Coded::from(&problem);
//! assert_eq!(coded.code, "html.element.stripped");
//! assert_eq!(coded.fields, [Field::Text("script".to_owned())]);
//! assert_eq!(Problem::try_from(coded), Ok(problem));
//! ```

use std::fmt;

use crate::{AnyCode, Bytes, ContentPath, Route, Span};

/// A code and its fields, in the order the catalog declares them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coded {
    /// `noun.noun.reason`.
    pub code: String,
    /// Every field, in catalog order.
    pub fields: Vec<Field>,
}

impl Coded {
    /// `code` with `fields`.
    pub fn new(code: impl Into<String>, fields: Vec<Field>) -> Self {
        Self { code: code.into(), fields }
    }

    /// Its fields, read in order by the generated conversions.
    pub(crate) fn into_fields(self) -> Fields {
        Fields { code: self.code, at: 0, rest: self.fields.into_iter() }
    }
}

/// One field's value, as its catalog type.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Field {
    /// `path`.
    Path(ContentPath),
    /// `paths`.
    Paths(Vec<ContentPath>),
    /// `route`.
    Route(Route),
    /// `text`.
    Text(String),
    /// `count`.
    Count(u64),
    /// `bytes`.
    Bytes(Bytes),
    /// `span`.
    Span(Span),
    /// `decision`.
    Decision(AnyCode),
}

impl Field {
    /// Its catalog type.
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Path(_) => Kind::Path,
            Self::Paths(_) => Kind::Paths,
            Self::Route(_) => Kind::Route,
            Self::Text(_) => Kind::Text,
            Self::Count(_) => Kind::Count,
            Self::Bytes(_) => Kind::Bytes,
            Self::Span(_) => Kind::Span,
            Self::Decision(_) => Kind::Decision,
        }
    }
}

/// A field's catalog type, without its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// One file.
    Path,
    /// Files, in path order.
    Paths,
    /// A route.
    Route,
    /// Text.
    Text,
    /// A number.
    Count,
    /// A size.
    Bytes,
    /// A byte range.
    Span,
    /// A decision code.
    Decision,
}

impl fmt::Display for Kind {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Path => "path",
            Self::Paths => "paths",
            Self::Route => "route",
            Self::Text => "text",
            Self::Count => "count",
            Self::Bytes => "bytes",
            Self::Span => "span",
            Self::Decision => "decision",
        })
    }
}

/// Implements `From<T> for Field` and `TryFrom<Field> for T` for each field type, the field
/// given back when it holds another. The same pair of impls over the catalog's eight types.
///
/// ```text
/// fields!(Path ContentPath);
/// // makes `impl From<ContentPath> for Field` and `impl TryFrom<Field> for ContentPath`
/// ```
macro_rules! fields {
    ($($variant:ident $ty:ty),+ $(,)?) => {
        $(
            impl From<$ty> for Field {
                fn from(value: $ty) -> Self {
                    Self::$variant(value)
                }
            }

            impl TryFrom<Field> for $ty {
                type Error = Field;

                fn try_from(field: Field) -> Result<Self, Field> {
                    match field {
                        Field::$variant(value) => Ok(value),
                        other => Err(other),
                    }
                }
            }
        )+
    };
}

fields!(
    Path ContentPath,
    Paths Vec<ContentPath>,
    Route Route,
    Text String,
    Count u64,
    Bytes Bytes,
    Span Span,
    Decision AnyCode,
);

/// Why a [`Coded`] is no problem or announcement.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refused {
    /// No catalog entry has the code.
    Unknown {
        /// The code given.
        code: String,
    },
    /// The entry has more fields than were given.
    Missing {
        /// The code.
        code: String,
        /// How many fields were given.
        given: usize,
    },
    /// A field holds another type than the entry declares.
    Mismatch {
        /// The code.
        code: String,
        /// The field's place, from 0.
        at: usize,
        /// The type it holds.
        found: Kind,
    },
    /// More fields were given than the entry has.
    Extra {
        /// The code.
        code: String,
        /// How many fields the entry has.
        wanted: usize,
    },
}

impl fmt::Display for Refused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown { code } => write!(out, "no catalog entry has the code `{code}`"),
            Self::Missing { code, given } => write!(out, "`{code}` has more fields than the {given} given"),
            Self::Mismatch { code, at, found } => write!(out, "field {at} of `{code}` is no {found}"),
            Self::Extra { code, wanted } => write!(out, "`{code}` has {wanted} fields, and more were given"),
        }
    }
}

impl std::error::Error for Refused {}

/// The fields of a [`Coded`], taken in order.
#[derive(Debug)]
pub(crate) struct Fields {
    code: String,
    at: usize,
    rest: std::vec::IntoIter<Field>,
}

impl Fields {
    /// Its code.
    pub(crate) fn code(&self) -> &str {
        &self.code
    }

    /// The next field, as `T`.
    pub(crate) fn next<T: TryFrom<Field, Error = Field>>(&mut self) -> Result<T, Refused> {
        let Some(field) = self.rest.next() else {
            return Err(Refused::Missing { code: self.code.clone(), given: self.at });
        };
        let at = self.at;
        self.at += 1;
        T::try_from(field).map_err(|field| Refused::Mismatch { code: self.code.clone(), at, found: field.kind() })
    }

    /// The value made from every field, refused when fields are left over.
    pub(crate) fn end<T>(mut self, made: T) -> Result<T, Refused> {
        match self.rest.next() {
            None => Ok(made),
            Some(_) => Err(Refused::Extra { code: self.code, wanted: self.at }),
        }
    }

    /// `code` is no catalog entry's.
    pub(crate) fn unknown<T>(self) -> Result<T, Refused> {
        Err(Refused::Unknown { code: self.code })
    }
}
