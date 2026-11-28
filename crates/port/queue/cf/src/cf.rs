//! The adapter.

use std::time::Duration;

use db::Db;
use logging::Field as _;
use model::Timestamp;
use queue::{Delivery, Message, Queue, Sent, Settled};
use queue_rows::{Error, Local};

use crate::wake::{Nudge, Wake};

type Failure<D> = queue::Error<Error<<D as Db>::Error>>;

/// The queue of the rows of a database, with a wake-up after what makes a message visible.
#[derive(Debug)]
pub struct Cf<D, W> {
    rows: Local<D>,
    wake: W,
}

impl<D: Db, W: Wake> Cf<D, W> {
    /// The rows `rows` are the queue, and `wake` nudges a consumer.
    pub const fn new(rows: Local<D>, wake: W) -> Self {
        Self { rows, wake }
    }

    /// The rows, which are the queue.
    pub const fn rows(&self) -> &Local<D> {
        &self.rows
    }

    /// The wake-up.
    pub const fn wake(&self) -> &W {
        &self.wake
    }

    /// Nudges a consumer to run after `delay`. A refusal is said and kept from the caller: the
    /// message is in the rows, so it is taken when a consumer next runs.
    async fn nudge(&self, delay: Duration) {
        let nudge = Nudge::after(delay);
        match self.wake.wake(nudge).await {
            Ok(()) => logging::debug!(
                name: logging::event::QUEUE_WOKEN,
                outcome = "sent",
                seconds = logging::Count(u64::from(nudge.seconds())).field(),
            ),
            Err(error) => logging::warn!(
                name: logging::event::QUEUE_WOKEN,
                outcome = "failed",
                error = logging::Failure(&error).field(),
            ),
        }
    }
}

impl<D: Db, W: Wake> Queue for Cf<D, W> {
    type Error = Error<D::Error>;

    async fn send(&self, message: Message, now: Timestamp) -> Result<Sent, Failure<D>> {
        let sent = self.rows.send(message, now).await?;
        // a duplicate is a message already in the rows, whose own send nudged
        if matches!(sent, Sent::Queued(_)) {
            self.nudge(Duration::ZERO).await;
        }
        Ok(sent)
    }

    async fn receive(&self, now: Timestamp, lease: Duration) -> Result<Option<Delivery>, Failure<D>> {
        self.rows.receive(now, lease).await
    }

    async fn ack(&self, delivery: &Delivery) -> Result<Settled, Failure<D>> {
        self.rows.ack(delivery).await
    }

    async fn release(&self, delivery: &Delivery, now: Timestamp, delay: Duration) -> Result<Settled, Failure<D>> {
        let settled = self.rows.release(delivery, now, delay).await?;
        if settled == Settled::Done {
            self.nudge(delay).await;
        }
        Ok(settled)
    }
}
