//! The queue port of the control plane: build requests, delivered at least once.
//!
//! - [`Queue::send`] adds a [`Message`]. A message with a [`Dedupe`] key is dropped if one with
//!   that key is still in the queue, so a webhook that retries does not build twice.
//! - [`Queue::receive`] hands out the oldest visible message and hides it for a lease. If it is
//!   not settled before the lease ends, it is delivered again with a higher attempt number.
//! - [`Queue::ack`] removes a message and [`Queue::release`] puts it back after a delay. Both
//!   refuse a stale [`Delivery`] (the lease ended and someone else holds it), so a slow worker
//!   cannot remove another worker's message.
//!
//! The clock is an argument, never read: the caller says what time it is, so a lease is tested
//! without sleeping. The adapters are `queue/local` and, hosted, `queue/cf`; the suite every
//! adapter runs is `queue/conformance`.

mod error;
mod message;
mod port;

pub use error::Error;
pub use message::{Dedupe, DedupeError, Delivery, Message, MessageId};
pub use port::{Queue, Sent, Settled};
