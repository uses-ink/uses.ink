//! What `ink pin` and `ink select` could not write into `ink.toml` or a page's frontmatter.

use std::fmt;

/// A change to a site's files that could not be made.
#[derive(Debug)]
#[non_exhaustive]
pub enum EditError {
    /// The pin could not be written into ink.toml.
    Pin {
        /// Why.
        source: config::PinsError,
    },
    /// A page's frontmatter cannot take or lose `publish`.
    Front {
        /// The page.
        path: model::ContentPath,
        /// Why.
        source: Box<format_markdown::Refusal>,
    },
    /// `include` could not be written into ink.toml.
    Include {
        /// Why.
        source: config::IncludeError,
    },
}

impl EditError {
    /// What kind of error it is, as `--json` names it.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Pin { .. } => "pin.config.unwritable",
            Self::Front { .. } => "select.frontmatter.refused",
            Self::Include { .. } => "select.include.unwritable",
        }
    }
}

impl fmt::Display for EditError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pin { .. } => write!(out, "the pin could not be written into ink.toml"),
            Self::Front { path, .. } => write!(out, "`publish` cannot be written into the frontmatter of {path}"),
            Self::Include { .. } => write!(out, "the selection could not be written into ink.toml"),
        }
    }
}

impl std::error::Error for EditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pin { source } => Some(source),
            Self::Front { source, .. } => Some(source.as_ref()),
            Self::Include { source } => Some(source),
        }
    }
}
