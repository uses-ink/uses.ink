//! What the shell fails with.

use std::fmt;

/// A body that is not a claim.
#[derive(Debug)]
pub struct Malformed {
    /// What the parser said.
    pub source: serde_json::Error,
}

impl fmt::Display for Malformed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the body is not a nonce claim")
    }
}

impl std::error::Error for Malformed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
