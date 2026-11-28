//! The name the TXT record is at.

use std::fmt;

use control::DomainName;

/// The host a proof is published at: `_uses-ink.<name>`. The leading label is the authority's
/// own, so the record never sits beside the name's own records.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Host(String);

impl Host {
    /// The label every proof host starts with.
    pub const LABEL: &'static str = "_uses-ink";

    /// The host of `name`'s proof.
    pub fn of(name: &DomainName) -> Self {
        Self(format!("{}.{name}", Self::LABEL))
    }

    /// The host as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Host {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}
