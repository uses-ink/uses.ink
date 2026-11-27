//! Writing the `include` list of `ink.toml`: what `ink select --folder` adds and `--remove` takes
//! out. Everything else in the file is left as it is, comments and spacing included.

use std::error::Error;
use std::fmt;

use toml_edit::{Array, DocumentMut, Item, Value};

use super::SyntaxError;
use crate::Pattern;

/// What is done to the `include` list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listing {
    /// The pattern joins the list.
    Add,
    /// The pattern leaves the list.
    Remove,
}

/// What an edit did to the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Listed {
    /// The pattern was added.
    Added,
    /// The pattern was already listed.
    Present,
    /// The pattern was removed.
    Removed,
    /// The pattern was not listed.
    Absent,
}

/// `ink.toml` after an edit of its `include` list.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Included {
    /// The file's text.
    pub text: String,
    /// What was done to the list.
    pub listed: Listed,
}

/// Why `include` could not be edited.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum IncludeError {
    /// The existing file does not parse.
    Syntax(SyntaxError),
    /// `include` holds something other than an array of strings.
    NotList,
}

impl fmt::Display for IncludeError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(_) => out.write_str("`include` cannot be written into ink.toml"),
            Self::NotList => out.write_str("`include` in ink.toml is not an array of strings"),
        }
    }
}

impl Error for IncludeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Syntax(error) => Some(error),
            Self::NotList => None,
        }
    }
}

/// The document whose `include` list is edited.
#[derive(Debug)]
pub(crate) struct Includes(DocumentMut);

impl Includes {
    /// See [`crate::include_into`].
    pub(crate) fn edit(existing: &str, pattern: &Pattern, listing: Listing) -> Result<Included, IncludeError> {
        let mut document = Self(existing.parse().map_err(|error| IncludeError::Syntax(SyntaxError(error)))?);
        let listed = match listing {
            Listing::Add => document.add(pattern)?,
            Listing::Remove => document.remove(pattern)?,
        };
        Ok(Included { text: document.0.to_string(), listed })
    }

    /// The array of strings `include` holds, if there is a key.
    fn list(&mut self) -> Result<Option<&mut Array>, IncludeError> {
        let Some(item) = self.0.get_mut("include") else { return Ok(None) };
        let array = item.as_value_mut().and_then(Value::as_array_mut).ok_or(IncludeError::NotList)?;
        if array.iter().any(|value| !value.is_str()) {
            return Err(IncludeError::NotList);
        }
        Ok(Some(array))
    }

    fn add(&mut self, pattern: &Pattern) -> Result<Listed, IncludeError> {
        let text = pattern.as_ref();
        let Some(array) = self.list()? else {
            self.0.insert("include", Item::Value(Array::from_iter([text]).into()));
            return Ok(Listed::Added);
        };
        if array.iter().any(|value| value.as_str() == Some(text)) {
            return Ok(Listed::Present);
        }
        // a list written one entry to a line keeps that: the new entry takes the last one's indent
        let indent = array.iter().last().and_then(|last| last.decor().prefix()).and_then(|prefix| prefix.as_str());
        let indent = indent.filter(|prefix| prefix.contains('\n')).map(str::to_owned);
        let mut value = Value::from(text);
        if let Some(indent) = indent {
            // a comment after the last entry's comma belongs to that entry's line, so the new
            // entry starts below it
            let trailing = array.trailing().as_str().unwrap_or_default().to_owned();
            match trailing.split_once('\n').filter(|(same, _)| !same.is_empty()) {
                Some((same, rest)) => {
                    value.decor_mut().set_prefix(format!("{same}\n{}", indent.trim_start_matches('\n')));
                    array.set_trailing(format!("\n{rest}"));
                }
                None => value.decor_mut().set_prefix(indent),
            }
        }
        array.push_formatted(value);
        Ok(Listed::Added)
    }

    fn remove(&mut self, pattern: &Pattern) -> Result<Listed, IncludeError> {
        let Some(array) = self.list()? else { return Ok(Listed::Absent) };
        let Some(at) = array.iter().position(|value| value.as_str() == Some(pattern.as_ref())) else {
            return Ok(Listed::Absent);
        };
        array.remove(at);
        Ok(Listed::Removed)
    }
}
