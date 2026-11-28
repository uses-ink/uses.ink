//! What an organization is called.

use std::fmt;
use std::str::FromStr;

/// The name of an organization: one line of text people read, 1 to 64 characters.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Name(String);

impl Name {
    /// The most characters a name has.
    pub const MAX: usize = 64;

    /// The name as written, trimmed.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// A name that cannot be one.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NameError {
    /// Nothing but space.
    Empty,
    /// More than [`Name::MAX`] characters.
    Long {
        /// How many it has.
        found: usize,
    },
    /// A control character, such as a line break, at this character.
    Control {
        /// Where.
        at: usize,
    },
}

impl fmt::Display for NameError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => out.write_str("an organization needs a name"),
            Self::Long { found } => write!(out, "a name has at most {} characters, this one has {found}", Name::MAX),
            Self::Control { at } => write!(out, "a name is one line of text: character {at} is a control character"),
        }
    }
}

impl std::error::Error for NameError {}

impl FromStr for Name {
    type Err = NameError;

    fn from_str(text: &str) -> Result<Self, NameError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(NameError::Empty);
        }
        if let Some((at, _)) = text.chars().enumerate().find(|(_, c)| c.is_control()) {
            return Err(NameError::Control { at });
        }
        let found = text.chars().count();
        if found > Self::MAX {
            return Err(NameError::Long { found });
        }
        Ok(Self(text.to_owned()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_name_is_one_trimmed_line_of_at_most_sixty_four_characters() {
        assert_eq!("  Garden club ".parse::<Name>().unwrap().as_str(), "Garden club");
        assert_eq!("   ".parse::<Name>(), Err(NameError::Empty));
        assert_eq!("a\nb".parse::<Name>(), Err(NameError::Control { at: 1 }));
        assert_eq!("x".repeat(65).parse::<Name>(), Err(NameError::Long { found: 65 }));
        assert!("x".repeat(64).parse::<Name>().is_ok());
    }
}
