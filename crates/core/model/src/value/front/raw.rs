//! The frontmatter value as the author wrote it.

use crate::{Expected, Span, Wanted};
use std::borrow::Cow;

/// One frontmatter value as the author wrote it, before any value type reads it. Each part
/// keeps its span; a number or a boolean keeps its text as written, so a key that wants text
/// reads `title: 2024` as `2024` and `title: 1.10` as `1.10`, not as a re-printed number.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Raw<'a> {
    /// The key with no value: `key:`, `key: null`, `key: ~`.
    Null {
        /// Where the value is (or, for a bare `key:`, the key).
        span: Span,
    },
    /// `true` or `false`.
    Bool {
        /// The value.
        value: bool,
        /// The text as written.
        written: &'a str,
        /// Where it is.
        span: Span,
    },
    /// A number, kept as written.
    Number {
        /// The text as written.
        written: &'a str,
        /// Where it is.
        span: Span,
    },
    /// Text, its quotes and escapes resolved.
    Text {
        /// The text.
        value: Cow<'a, str>,
        /// Where it is, quotes included.
        span: Span,
    },
    /// A list, each item with its own span.
    List {
        /// The items, in order.
        items: Vec<Self>,
        /// From the list's start to the end of its last item.
        span: Span,
    },
    /// A mapping. No value a decision reads is one; it is kept to be refused with its span.
    Map {
        /// Where it is.
        span: Span,
    },
}

impl Raw<'_> {
    /// Where the value is.
    pub const fn span(&self) -> Span {
        match self {
            Self::Null { span }
            | Self::Bool { span, .. }
            | Self::Number { span, .. }
            | Self::Text { span, .. }
            | Self::List { span, .. }
            | Self::Map { span } => *span,
        }
    }

    /// The value as text: the text itself, or a number or boolean as written. `None` for a
    /// null, a list or a mapping.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text { value, .. } => Some(value),
            Self::Bool { written, .. } | Self::Number { written, .. } => Some(written),
            Self::Null { .. } | Self::List { .. } | Self::Map { .. } => None,
        }
    }

    /// The value as text, trimmed, or the refusal of it as not `wanted`.
    pub fn scalar(&self, wanted: Wanted) -> Result<&str, Expected> {
        self.text().map(str::trim).ok_or_else(|| Expected::new(wanted, self.span()))
    }
}
