//! The catalog the wording is generated from, parsed: what `ink help <code>` reads.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;

/// `diag/catalog.toml` as it was at build time.
const SOURCE: &str = include_str!("../catalog.toml");

/// The catalog's diagnostics and notices by code.
#[derive(Debug, Default, Deserialize)]
#[non_exhaustive]
pub struct Catalog {
    /// Every diagnostic, by its code.
    #[serde(default)]
    pub diagnostic: BTreeMap<String, Wording>,
    /// Every notice, by its code.
    #[serde(default)]
    pub notice: BTreeMap<String, Wording>,
}

/// One entry of the catalog, its templates unfilled.
#[derive(Debug, Deserialize)]
#[non_exhaustive]
pub struct Wording {
    /// How bad it is; a notice has none.
    pub severity: Option<String>,
    /// Whose move it is; a notice has none.
    pub fault: Option<String>,
    /// What it says.
    pub message: String,
    /// What to do about it, when the catalog says.
    pub fix: Option<String>,
}

impl Catalog {
    /// The catalog this crate was built with.
    ///
    /// # Errors
    ///
    /// [`Unreadable`] when it is not valid, which `cargo xtask diag` refuses first.
    pub fn read() -> Result<Self, Unreadable> {
        toml::from_str(SOURCE).map_err(Unreadable)
    }
}

/// The catalog this crate carries does not parse. It says what the parser said.
#[derive(Debug)]
pub struct Unreadable(toml::de::Error);

impl fmt::Display for Unreadable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(out)
    }
}

impl std::error::Error for Unreadable {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        std::error::Error::source(&self.0)
    }
}
