//! What a decision, a diagnostic or a notice is about, with its kind erased: the form storage,
//! the wire and diagnostics use. Typed code names its subject kind instead
//! ([`decision::Subject`](crate::decision::Subject)).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ContentPath;
use crate::decision::Scope;

/// What a decision is about.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Subject {
    /// The whole site.
    Site,
    /// One file of the snapshot.
    File(ContentPath),
}

impl Subject {
    /// Whether it is the site or a file.
    pub const fn scope(&self) -> Scope {
        match self {
            Self::Site => Scope::Site,
            Self::File(_) => Scope::File,
        }
    }
}

impl fmt::Display for Subject {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Site => out.write_str("the site"),
            Self::File(path) => write!(out, "{path}"),
        }
    }
}
