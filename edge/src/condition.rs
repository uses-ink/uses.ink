//! The headers that change how a file is answered: `If-None-Match`, `Range`, `If-Range` and
//! `Accept-Encoding`.

use std::fmt;

use crate::encoding::Accepted;
use crate::range::Ranges;
use crate::tag::{Matching, Validator};

/// What the client said about the copy it holds and the bytes it wants.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Conditions {
    /// `If-None-Match`.
    pub none_match: Option<Matching>,
    /// `Range`.
    pub range: Option<Ranges>,
    /// `If-Range`.
    pub if_range: Option<Validator>,
    /// `Accept-Encoding`.
    pub accept: Accepted,
}

/// Why a conditional header is refused. Each carries the byte offset in the value where it went
/// wrong.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConditionError {
    /// An `If-None-Match` that is neither `*` nor a list of entity tags.
    IfNoneMatch {
        /// The byte offset.
        at: usize,
    },
    /// An `If-None-Match` listing more tags than the limit.
    Tags {
        /// How many.
        count: usize,
        /// The limit.
        limit: usize,
    },
    /// A `Range` that is not `unit=` and ranges, or whose numbers do not fit or run backwards.
    Range {
        /// The byte offset.
        at: usize,
    },
    /// An `If-Range` that is neither an entity tag nor a date.
    IfRange {
        /// The byte offset.
        at: usize,
    },
    /// An `Accept-Encoding` with a coding or a weight that does not parse.
    AcceptEncoding {
        /// The byte offset.
        at: usize,
    },
}

impl fmt::Display for ConditionError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IfNoneMatch { at } => write!(out, "if-none-match is not a list of entity tags (offset {at})"),
            Self::Tags { count, limit } => write!(out, "if-none-match lists {count} tags, over the limit of {limit}"),
            Self::Range { at } => write!(out, "range is not a list of byte ranges (offset {at})"),
            Self::IfRange { at } => write!(out, "if-range is neither an entity tag nor a date (offset {at})"),
            Self::AcceptEncoding { at } => write!(out, "accept-encoding does not parse (offset {at})"),
        }
    }
}

impl std::error::Error for ConditionError {}
