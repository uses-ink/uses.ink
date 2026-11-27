//! A language tag, as `<html lang>` and feeds write it.

use std::fmt;
use std::ops::RangeInclusive;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Letters in a primary language: ISO 639 codes are two or three letters long.
const PRIMARY_LENGTHS: RangeInclusive<usize> = 2..=3;
/// Letters or digits in a subtag: BCP 47 allows one to eight.
const SUBTAG_LENGTHS: RangeInclusive<usize> = 1..=8;

/// A BCP 47 language tag in the shape sites use: a primary language of two or three letters,
/// then subtags of letters or digits (`en`, `fr-CH`, `zh-Hant-TW`). Case is kept as written.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct Language(String);

/// Why text is not a language tag.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LanguageError {
    /// The primary language is not two or three letters.
    Primary,
    /// The subtag at this place (from 1) is empty, too long or not letters and digits.
    Subtag {
        /// Which subtag, from 1.
        index: usize,
    },
}

impl fmt::Display for LanguageError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primary => out.write_str("a language tag starts with two or three letters (`en`, `fra`)"),
            Self::Subtag { index } => write!(out, "subtag {index} is not one to eight letters or digits"),
        }
    }
}

impl std::error::Error for LanguageError {}

impl FromStr for Language {
    type Err = LanguageError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut parts = text.split('-');
        let primary = parts.next().unwrap_or_default();
        if !PRIMARY_LENGTHS.contains(&primary.len()) || !primary.bytes().all(|b| b.is_ascii_alphabetic()) {
            return Err(LanguageError::Primary);
        }
        for (index, part) in parts.enumerate() {
            if !SUBTAG_LENGTHS.contains(&part.len()) || !part.bytes().all(|b| b.is_ascii_alphanumeric()) {
                return Err(LanguageError::Subtag { index: index + 1 });
            }
        }
        Ok(Self(text.to_owned()))
    }
}

impl TryFrom<String> for Language {
    type Error = LanguageError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<Language> for String {
    fn from(language: Language) -> Self {
        language.0
    }
}

impl AsRef<str> for Language {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Language {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_sites_use() {
        for good in ["en", "fr-CH", "zh-Hant-TW", "gsw", "es-419"] {
            assert!(good.parse::<Language>().is_ok(), "{good}");
        }
        assert_eq!("english".parse::<Language>(), Err(LanguageError::Primary));
        assert_eq!("en-".parse::<Language>(), Err(LanguageError::Subtag { index: 1 }));
        assert_eq!("en_US".parse::<Language>(), Err(LanguageError::Primary));
    }
}
