//! What the operator sets about how the control plane behaves: how long a handle is held, and
//! the budgets that join it later. Every value has a default that is written down.

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
}

impl Policy {
    /// The defaults: a hold of twelve months and the default [`Limits`].
    pub const DEFAULT: Self = Self { hold: Hold::DEFAULT, limits: Limits::DEFAULT };

    /// The same policy holding a handle for `hold`.
    #[must_use]
    pub const fn holding(mut self, hold: Duration) -> Self {
        self.hold = hold;
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
