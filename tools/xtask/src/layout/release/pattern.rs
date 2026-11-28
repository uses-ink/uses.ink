//! A tag pattern, read as GitHub reads the `tags:` of a workflow.

use std::fmt;

/// A pattern of `on: push: tags:`.
#[derive(Debug, Clone, Copy)]
pub(super) struct Pattern<'a>(&'a str);

impl<'a> From<&'a str> for Pattern<'a> {
    fn from(text: &'a str) -> Self {
        Self(text)
    }
}

/// Why GitHub refuses a pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Reject {
    /// A `[` with no `]` after it.
    Unclosed,
    /// A character in `[ ]` that is not a letter, a digit or the `-` of a range: GitHub allows
    /// only ranges of `a-z`, `A-Z` and `0-9` there, so `[0-9A-Za-z.]` fails the whole file.
    Class(char),
}

impl fmt::Display for Reject {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unclosed => write!(out, "a `[` is never closed"),
            Self::Class(found) => write!(out, "`{found}` is not allowed in `[ ]`, only letters, digits and ranges"),
        }
    }
}

impl std::error::Error for Reject {}

impl Pattern<'_> {
    /// The first thing in it GitHub refuses, if any.
    pub(super) fn reject(self) -> Option<Reject> {
        let mut inside = false;
        for found in self.0.chars() {
            match (inside, found) {
                (false, '[') => inside = true,
                (true, ']') => inside = false,
                (true, other) if !(other.is_ascii_alphanumeric() || other == '-') => return Some(Reject::Class(other)),
                _ => {}
            }
        }
        inside.then_some(Reject::Unclosed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_of_ranges_pass_and_anything_else_in_them_is_named() {
        assert_eq!(Pattern::from("v[0-9]+.[0-9]+.[0-9]+").reject(), None);
        assert_eq!(Pattern::from("v1.0.0-[0-9A-Za-z]+").reject(), None);
        assert_eq!(Pattern::from("v[0-9A-Za-z.]+").reject(), Some(Reject::Class('.')));
        assert_eq!(Pattern::from("v[0-9").reject(), Some(Reject::Unclosed));
        assert_eq!(Pattern::from("v[[0-9]]").reject(), Some(Reject::Class('[')));
    }
}
