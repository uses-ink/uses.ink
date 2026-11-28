//! The receipt of a removal.

use std::fmt;

use jiff::Timestamp;
use model::Bytes;

use crate::Handle;
use crate::id::{Account, Id, Site};

/// What a removal removed (decision 0022). The edge sends it to the account's address; what is
/// in it is counts and names, never content.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Receipt {
    /// The site removed.
    pub site: Id<Site>,
    /// Who owned it.
    pub account: Id<Account>,
    /// The handle, which stays unclaimable.
    pub handle: Handle,
    /// When the removal finished.
    pub removed: Timestamp,
    /// How many deploys it had.
    pub deploys: u64,
    /// How many blobs were deleted by this run, and how many bytes they held. A run that
    /// finishes an earlier, interrupted one counts only what was left.
    pub blobs: u64,
    /// The bytes those blobs held.
    pub bytes: Bytes,
    /// How many builds it had.
    pub builds: u64,
    /// How many credentials it had.
    pub credentials: u64,
}

impl fmt::Display for Receipt {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "removed site {} ({}) at {}: {} deploys, {} blobs ({}), {} builds, {} credentials",
            self.site, self.handle, self.removed, self.deploys, self.blobs, self.bytes, self.builds, self.credentials
        )
    }
}
