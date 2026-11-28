//! The queue port hosted on Cloudflare (decision 0132).
//!
//! Cloudflare Queues has no dedupe key and no lease a Worker can set, so it cannot be the queue
//! (decision 0079). The queue is the rows of `queue-rows` over D1, as when self-hosted, and a
//! Queues message is only a **wake-up**: after a message is queued, [`Cf`] sends a [`Nudge`]
//! through a [`Wake`], and the consumer that runs on it reads the rows. The rows stay the truth: a
//! wake-up that is lost, late or doubled changes nothing but when a message is taken, and a Cron
//! trigger covers a lost one. A wake-up that fails is a `queue.woken` warning and never the answer
//! of `send`.
//!
//! [`Queues`] is the wake-up through a Queues binding (wasm32 only); [`Silent`] sends none, for a
//! deployment that wakes on Cron alone; [`Recorder`] is the fake the cases use.

mod cases;
mod cf;
mod recorder;
mod wake;

#[cfg(target_arch = "wasm32")]
mod binding;

#[cfg(target_arch = "wasm32")]
pub use binding::Queues;
pub use cases::{Nudged, Refused};
pub use cf::Cf;
pub use recorder::{Recorder, Refusal};
pub use wake::{Nudge, Silent, Wake};
