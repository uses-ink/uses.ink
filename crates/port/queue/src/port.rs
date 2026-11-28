//! The port itself.

use std::time::Duration;

use model::Timestamp;

use crate::{Delivery, Error, Message, MessageId};

/// What sending did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Sent {
    /// The message is in the queue under this id.
    Queued(MessageId),
    /// A message with the same dedupe key is already in the queue, under this id; nothing was added.
    Duplicate(MessageId),
}

/// What settling a delivery did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Settled {
    /// The delivery was current and is settled.
    Done,
    /// The lease had ended, and the message was gone or delivered again. Nothing changed.
    Stale,
}

/// A queue with at-least-once delivery. Futures are not required to be `Send`.
#[expect(
    async_fn_in_trait,
    reason = "Cloudflare Queues futures are not Send on Workers, so no Send bound can be promised"
)]
pub trait Queue {
    /// The adapter's own error.
    type Error: std::error::Error + 'static;

    /// Adds `message`, visible from `now`.
    async fn send(&self, message: Message, now: Timestamp) -> Result<Sent, Error<Self::Error>>;

    /// The oldest message visible at `now`, hidden from other consumers until `now + lease`.
    async fn receive(&self, now: Timestamp, lease: Duration) -> Result<Option<Delivery>, Error<Self::Error>>;

    /// Removes the delivered message: it was handled.
    async fn ack(&self, delivery: &Delivery) -> Result<Settled, Error<Self::Error>>;

    /// Makes the delivered message visible again at `now + delay`, keeping its attempt count:
    /// it was not handled, and may be tried later.
    async fn release(
        &self,
        delivery: &Delivery,
        now: Timestamp,
        delay: Duration,
    ) -> Result<Settled, Error<Self::Error>>;
}
