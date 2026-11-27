//! What the lookup may keep and for how long.

use std::time::Duration;

/// How the lookup behaves. Every value is the operator's to set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// How long a handle's live deploy is remembered: a deploy that went live or a rollback is
    /// seen within this long. Zero asks the control plane every time.
    pub ttl: Duration,
    /// How many decoded manifests are kept. Zero keeps none.
    pub deploys: usize,
    /// How many lookups may wait for the control plane before a caller waits for room.
    pub waiting: usize,
}

impl Settings {
    /// Ten seconds: what architecture section 11 says of the lookup per colo.
    pub const TTL: Duration = Duration::from_secs(10);
}

impl Default for Settings {
    fn default() -> Self {
        Self { ttl: Self::TTL, deploys: 64, waiting: 64 }
    }
}
