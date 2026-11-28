//! The components `ink build --check` runs.

use std::fmt;
use std::path::PathBuf;

/// A component that could not be found, read or loaded.
#[derive(Debug)]
#[non_exhaustive]
pub enum ComponentError {
    /// A component `ink build --check` runs could not be read.
    Component {
        /// Its file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// A component this `ink` does not carry: it was built without them.
    Unbundled {
        /// The plugin id asked for.
        id: String,
    },
    /// A component could not be loaded.
    Plugin {
        /// Why.
        source: host_wasmtime::Error,
    },
}

impl ComponentError {
    /// What kind of error it is, as `--json` names it.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Self::Component { .. } => "component.file.unreadable",
            Self::Unbundled { .. } => "component.bundled.missing",
            Self::Plugin { .. } => "component.load.failed",
        }
    }
}

impl fmt::Display for ComponentError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Component { path, .. } => write!(out, "the component {} could not be read", path.display()),
            Self::Unbundled { id } => write!(out, "this ink carries no component `{id}`: it was built without them"),
            Self::Plugin { .. } => write!(out, "a component could not be loaded"),
        }
    }
}

impl std::error::Error for ComponentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Component { source, .. } => Some(source),
            Self::Plugin { source } => Some(source),
            Self::Unbundled { .. } => None,
        }
    }
}
