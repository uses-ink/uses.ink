//! A stored column that does not read back as the value it should be.

use std::fmt;

use crate::id::IdError;

/// A stored column that does not read back as the value it should be: the database was changed by
/// something other than this code, or by another version of it.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unreadable {
    /// The column is missing or of the wrong kind.
    Column(db::ColumnError),
    /// The column holds a bad id.
    Id {
        /// The column.
        column: &'static str,
        /// Why.
        source: IdError,
    },
    /// The column holds a bad deploy id.
    Deploy {
        /// The column.
        column: &'static str,
        /// Why.
        source: serde::de::value::Error,
    },
    /// The column holds a bad hash.
    Hash {
        /// The column.
        column: &'static str,
        /// Why.
        source: model::HashError,
    },
    /// The column holds a time that does not exist.
    Time {
        /// The column.
        column: &'static str,
        /// Why.
        source: jiff::Error,
    },
    /// The column holds a name this version does not know, such as a state.
    Name {
        /// The column.
        column: &'static str,
        /// What it holds.
        found: String,
    },
    /// The column holds an email address that is no longer valid.
    Address {
        /// The column.
        column: &'static str,
        /// Why.
        source: mail::AddressError,
    },
    /// The column holds a digest that is not 32 bytes.
    Digest {
        /// The column.
        column: &'static str,
    },
    /// The column holds bytes that are not the one spelling or size of a key's value.
    Encoding {
        /// The column.
        column: &'static str,
        /// Why.
        source: sig::Undecodable,
    },
    /// The column holds text that is no longer a valid label, code or date.
    Text {
        /// The column.
        column: &'static str,
    },
}

impl fmt::Display for Unreadable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Column(_) => out.write_str("a stored row has a column that cannot be read"),
            Self::Id { column, .. } => write!(out, "column `{column}` holds a bad id"),
            Self::Deploy { column, .. } => write!(out, "column `{column}` holds a bad deploy id"),
            Self::Hash { column, .. } => write!(out, "column `{column}` holds a bad hash"),
            Self::Time { column, .. } => write!(out, "column `{column}` holds a bad time"),
            Self::Name { column, found } => {
                write!(out, "column `{column}` holds `{found}`, which this version does not know")
            }
            Self::Address { column, .. } => write!(out, "column `{column}` holds an address that is no longer valid"),
            Self::Digest { column } => write!(out, "column `{column}` holds a digest that is not 32 bytes"),
            Self::Encoding { column, .. } => write!(out, "column `{column}` holds a key value that is not valid"),
            Self::Text { column } => write!(out, "column `{column}` holds text that is no longer valid"),
        }
    }
}

impl std::error::Error for Unreadable {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Column(source) => Some(source),
            Self::Id { source, .. } => Some(source),
            Self::Deploy { source, .. } => Some(source),
            Self::Hash { source, .. } => Some(source),
            Self::Time { source, .. } => Some(source),
            Self::Address { source, .. } => Some(source),
            Self::Encoding { source, .. } => Some(source),
            Self::Name { .. } | Self::Digest { .. } | Self::Text { .. } => None,
        }
    }
}

impl From<db::ColumnError> for Unreadable {
    fn from(source: db::ColumnError) -> Self {
        Self::Column(source)
    }
}
