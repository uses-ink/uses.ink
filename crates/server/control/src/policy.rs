//! What the operator sets about how the control plane behaves: how long a handle is held, and
//! the budgets and how a removal is bounded. Every value has a default that is written down.

use std::time::Duration;

use crate::{Hold, Limits};

/// The operator's settings of a [`Control`](crate::Control).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Policy {
    /// How long a handle is held after its site left it (`--handle-hold`).
    pub hold: Duration,
    /// The budgets (`--limit-stored` and the flags beside it).
    pub limits: Limits,
    /// The most blobs one step of a removal deletes (`--remove-step`).
    pub removal: u64,
    /// How long a preview of a branch is served (`--preview-days`).
    pub preview: Duration,
    /// The most custom domains a site has, proved and not (`--limit-domains`).
    pub domains: u64,
    /// The most abuse reports open at once, over every source (`--limit-reports`). A report is
    /// taken from anyone, and the per-source rate bounds only one source at a time, so this bounds
    /// what many sources together can store. A report the operator actioned no longer counts.
    pub reports: u64,
    /// How long a blob that nothing names any more stays before it is deleted (`--blob-grace`).
    /// It must outlast the time between a build's upload and its recorded deploy.
    pub grace: Duration,
}

impl Policy {
    /// The defaults: a hold of twelve months and the default [`Limits`].
    pub const DEFAULT: Self = Self {
        hold: Hold::DEFAULT,
        limits: Limits::DEFAULT,
        removal: Self::STEP,
        preview: Self::PREVIEW,
        domains: Self::DOMAINS,
        reports: Self::REPORTS,
        grace: Self::GRACE,
    };

    /// How long a preview lasts unless the operator says: seven days (`docs/hosted.md` HQ12).
    pub const PREVIEW: Duration = Duration::from_hours(7 * 24);

    /// How many custom domains a site has, proved and not, unless the operator says: ten.
    pub const DOMAINS: u64 = 10;

    /// How long an unnamed blob stays unless the operator says: fifteen minutes, longer than the
    /// default build time of ten (decision 0150).
    pub const GRACE: Duration = Duration::from_mins(15);

    /// How many abuse reports are open at once unless the operator says: ten thousand.
    pub const REPORTS: u64 = 10_000;

    /// How many blobs a step of a removal deletes unless the operator says.
    pub const STEP: u64 = 500;

    /// The same policy holding a handle for `hold`.
    #[must_use]
    pub const fn holding(mut self, hold: Duration) -> Self {
        self.hold = hold;
        self
    }

    /// The same policy deleting at most `blobs` in a step of a removal.
    #[must_use]
    pub const fn stepping(mut self, blobs: u64) -> Self {
        self.removal = blobs;
        self
    }

    /// The same policy serving a preview for `preview`.
    #[must_use]
    pub const fn previewing(mut self, preview: Duration) -> Self {
        self.preview = preview;
        self
    }

    /// The same policy letting a site have `domains` custom domains.
    #[must_use]
    pub const fn domained(mut self, domains: u64) -> Self {
        self.domains = domains;
        self
    }

    /// The same policy keeping at most `reports` abuse reports open.
    #[must_use]
    pub const fn reported(mut self, reports: u64) -> Self {
        self.reports = reports;
        self
    }

    /// The same policy keeping an unnamed blob for `grace` before it is deleted.
    #[must_use]
    pub const fn graced(mut self, grace: Duration) -> Self {
        self.grace = grace;
        self
    }

    /// The same policy under `limits`.
    #[must_use]
    pub const fn limited(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
