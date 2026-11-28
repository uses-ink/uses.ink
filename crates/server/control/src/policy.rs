//! What the operator sets about how the control plane behaves: how long a handle is held, and
//! the budgets that join it later. Every value has a default that is written down.

use std::time::Duration;

use crate::Hold;

/// The operator's settings of a [`Control`](crate::Control).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Policy {
    /// How long a handle is held after its site left it (`--handle-hold`).
    pub hold: Duration,
}

impl Policy {
    /// The defaults: a hold of twelve months.
    pub const DEFAULT: Self = Self { hold: Hold::DEFAULT };

    /// The same policy holding a handle for `hold`.
    #[must_use]
    pub const fn holding(mut self, hold: Duration) -> Self {
        self.hold = hold;
        self
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::DEFAULT
    }
}
