//! The values a statement binds and a row holds: what D1 can carry.

use std::fmt;

/// One value of a column or a bound parameter.
#[derive(Clone, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// SQL `NULL`.
    Null,
    /// A 64-bit integer. Booleans are stored as 0 and 1.
    Integer(i64),
    /// A 64-bit float.
    Real(f64),
    /// UTF-8 text.
    Text(String),
    /// Bytes.
    Blob(Vec<u8>),
}

/// The kind of a [`Value`], for saying what was expected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Kind {
    /// [`Value::Null`].
    Null,
    /// [`Value::Integer`].
    Integer,
    /// [`Value::Real`].
    Real,
    /// [`Value::Text`].
    Text,
    /// [`Value::Blob`].
    Blob,
}

impl fmt::Display for Kind {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Null => "null",
            Self::Integer => "an integer",
            Self::Real => "a real number",
            Self::Text => "text",
            Self::Blob => "bytes",
        })
    }
}

impl Value {
    /// Which kind this value is.
    pub const fn kind(&self) -> Kind {
        match self {
            Self::Null => Kind::Null,
            Self::Integer(_) => Kind::Integer,
            Self::Real(_) => Kind::Real,
            Self::Text(_) => Kind::Text,
            Self::Blob(_) => Kind::Blob,
        }
    }
}

impl fmt::Debug for Value {
    /// Bytes show only their length: a blob may be a sealed credential or a person's content.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => out.write_str("Null"),
            Self::Integer(n) => write!(out, "Integer({n})"),
            Self::Real(n) => write!(out, "Real({n})"),
            Self::Text(text) => write!(out, "Text({text:?})"),
            Self::Blob(bytes) => write!(out, "Blob({} bytes)", bytes.len()),
        }
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Self::Integer(n)
    }
}

impl From<u32> for Value {
    fn from(n: u32) -> Self {
        Self::Integer(n.into())
    }
}

impl From<bool> for Value {
    fn from(flag: bool) -> Self {
        Self::Integer(flag.into())
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Self::Real(n)
    }
}

impl From<&str> for Value {
    fn from(text: &str) -> Self {
        Self::Text(text.to_owned())
    }
}

impl From<String> for Value {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

impl From<Vec<u8>> for Value {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Blob(bytes)
    }
}

impl From<&[u8]> for Value {
    fn from(bytes: &[u8]) -> Self {
        Self::Blob(bytes.to_vec())
    }
}

impl<T: Into<Self>> From<Option<T>> for Value {
    fn from(value: Option<T>) -> Self {
        value.map_or(Self::Null, Into::into)
    }
}

/// A value a column reader could not take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mismatch {
    /// The value is of another kind than the reader wanted.
    Kind {
        /// What the reader wanted.
        wanted: Kind,
        /// What the value was.
        found: Kind,
    },
    /// An integer outside the range of the type the reader builds.
    Range {
        /// The integer.
        found: i64,
    },
}

impl Mismatch {
    /// `value` was not of the `wanted` kind.
    pub const fn of(wanted: Kind, value: &Value) -> Self {
        Self::Kind { wanted, found: value.kind() }
    }
}

impl fmt::Display for Mismatch {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kind { wanted, found } => write!(out, "wanted {wanted}, found {found}"),
            Self::Range { found } => write!(out, "the integer {found} is out of range"),
        }
    }
}

impl std::error::Error for Mismatch {}
