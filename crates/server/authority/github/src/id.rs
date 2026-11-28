//! The numbers GitHub gives an App and its installations.

use std::fmt;

/// The id of the registered GitHub App: the `iss` of its JWT.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AppId(pub u64);

impl fmt::Display for AppId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)
    }
}

/// The id of one installation of the App on an account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Deserialize)]
#[serde(transparent)]
pub struct InstallationId(pub u64);

impl fmt::Display for InstallationId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)
    }
}
