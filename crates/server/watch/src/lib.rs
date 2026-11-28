//! The loops a self-host runs beside the edge (architecture section 12, decision 0052).
//!
//! - [`Watch`] polls the git sources of the sites that have one. A [`Heads`] port says where a
//!   repository is now; when that is not what was seen last, the watch queues a build through
//!   the control plane with the trigger `poll` and a dedupe key of site and commit. The
//!   [`Interval`] is configurable, has a floor so no setting makes a busy loop, and backs off
//!   for a source that does not move or does not answer.
//! - [`Worker`] takes build requests off the queue and runs them, resting when there are none.
//!
//! Both read time and ids only through the [`clock`] ports, and both wait with
//! [`clock::Clock::sleep`], so a test runs a loop for a fixed number of rounds in no time.

mod access;
mod head;
mod interval;
mod watch;
mod worker;

pub use access::{Access, AccessError};
pub use head::{Head, HeadError, Heads};
pub use interval::{Interval, IntervalError};
pub use watch::{Outcome, Polled, Report, Round, Watch};
pub use worker::{Done, Worker};
