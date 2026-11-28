//! The brand list: names a person cannot claim as a handle (`docs/hosted.md` HQ11). The list is
//! the data file `data/reserved-handles.toml`, read once and checked: every name must be a handle.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::{Handle, HandleError};

/// The file as it was at build time.
const SOURCE: &str = include_str!("../data/reserved-handles.toml");

/// The names of the brand list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Brands(BTreeSet<Handle>);

/// The file's shape.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    names: Vec<String>,
}

/// Why the brand list does not read.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BrandError {
    /// The file is not the TOML of a list of names.
    Syntax(toml::de::Error),
    /// A name of the list is no handle.
    Name {
        /// The name, as written.
        written: String,
        /// Why it is no handle.
        source: HandleError,
    },
}

impl fmt::Display for BrandError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(_) => out.write_str("the brand list is not a TOML list of names"),
            Self::Name { written, .. } => write!(out, "`{written}` in the brand list is no handle"),
        }
    }
}

impl std::error::Error for BrandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Syntax(source) => Some(source),
            Self::Name { source, .. } => Some(source),
        }
    }
}

impl Brands {
    /// The list read from `text`.
    pub fn read(text: &str) -> Result<Self, BrandError> {
        let file: File = toml::from_str(text).map_err(BrandError::Syntax)?;
        let mut names = BTreeSet::new();
        for written in file.names {
            let handle = written.parse::<Handle>().map_err(|source| BrandError::Name { written, source })?;
            names.insert(handle);
        }
        Ok(Self(names))
    }

    /// The list this build carries, read once.
    pub fn bundled() -> Result<&'static Self, BrandError> {
        static LIST: OnceLock<Result<Brands, BrandError>> = OnceLock::new();
        LIST.get_or_init(|| Self::read(SOURCE)).as_ref().map_err(Clone::clone)
    }

    /// Whether `handle` is on the list.
    pub fn holds(&self, handle: &Handle) -> bool {
        self.0.contains(handle)
    }

    /// How many names the list has.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_bundled_list_reads_and_holds_no_infrastructure_name() {
        let list = Brands::bundled().unwrap();
        assert!(!list.is_empty());
        for name in Handle::RESERVED {
            assert!(!list.holds(&name.parse().unwrap()), "{name} is in both lists");
        }
    }

    #[test]
    fn a_name_that_is_no_handle_is_a_typed_error() {
        let error = Brands::read(r#"names = ["ok-name", "Bad"]"#).unwrap_err();
        assert!(
            matches!(&error, BrandError::Name { written, source: HandleError::Character { at: 0 } } if written == "Bad")
        );
        assert!(matches!(Brands::read("names = 3").unwrap_err(), BrandError::Syntax(_)));
        assert!(matches!(Brands::read("other = []").unwrap_err(), BrandError::Syntax(_)));
    }
}
