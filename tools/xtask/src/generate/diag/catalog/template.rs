//! The templates of an entry: its message and its fix, as literal text and fields.

use super::Key;

/// A message or fix: literal text and fields, in order.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Template(pub(crate) Vec<Piece>);

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Piece {
    Text(String),
    Field(String),
}

impl Template {
    /// The fields it uses, each once, in first-use order.
    pub(crate) fn fields(&self) -> Vec<&str> {
        let mut used: Vec<&str> = Vec::new();
        for piece in &self.0 {
            if let Piece::Field(name) = piece
                && !used.contains(&name.as_str())
            {
                used.push(name);
            }
        }
        used
    }
}

/// Which template of an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    Message,
    Fix,
}

impl Part {
    /// The key it is written under.
    pub(super) const fn key(self) -> Key {
        match self {
            Self::Message => Key::Message,
            Self::Fix => Key::Fix,
        }
    }
}
