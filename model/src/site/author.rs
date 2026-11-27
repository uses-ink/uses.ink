//! Who wrote a page.

use serde::{Deserialize, Serialize};

/// Someone who wrote the site, by the name they go by.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Author(String);

/// An author's name was empty or only spaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyAuthor;

impl std::fmt::Display for EmptyAuthor {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("an author's name has at least one character that is not a space")
    }
}

impl std::error::Error for EmptyAuthor {}

impl TryFrom<String> for Author {
    type Error = EmptyAuthor;

    /// The name, trimmed.
    fn try_from(name: String) -> Result<Self, Self::Error> {
        let trimmed = name.trim();
        if trimmed.is_empty() { Err(EmptyAuthor) } else { Ok(Self(trimmed.to_owned())) }
    }
}

impl From<Author> for String {
    fn from(author: Author) -> Self {
        author.0
    }
}

impl AsRef<str> for Author {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Author {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(&self.0)
    }
}
