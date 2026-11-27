//! Mail waiting to be sent, and what sends it.
//!
//! A sign-in call does not send mail: it puts the message in the [`Outbox`] and answers. A
//! [`Courier`] on the same task as the API takes messages out and hands them to a
//! [`Mailer`]. The answer to "send me a link" therefore takes the same time whether or not
//! there was anything to send, which is most of what makes it timing-flat. The outbox is in
//! memory and bounded: a message lost to a restart or a full box is a link that never arrives,
//! and the person asks again.

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use clock::{Clock, Slept};
use mail::{Mailer, Message};

/// The outbox is full: the message was dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Full {
    /// How many messages it holds.
    pub capacity: usize,
}

impl fmt::Display for Full {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "the outbox holds {} messages and is full", self.capacity)
    }
}

impl std::error::Error for Full {}

#[derive(Debug)]
struct Inner {
    queue: Mutex<VecDeque<Message>>,
    capacity: usize,
    dropped: AtomicU64,
}

/// A bounded queue of messages, shared by the API that fills it and the courier that empties it.
#[derive(Clone, Debug)]
pub struct Outbox(Arc<Inner>);

impl Outbox {
    /// An outbox that holds `capacity` messages.
    pub fn new(capacity: usize) -> Self {
        Self(Arc::new(Inner { queue: Mutex::new(VecDeque::new()), capacity, dropped: AtomicU64::new(0) }))
    }

    /// Adds a message, or drops it and counts it when the box is full.
    pub fn push(&self, message: Message) -> Result<(), Full> {
        let mut queue = self.0.queue.lock().unwrap_or_else(PoisonError::into_inner);
        if queue.len() >= self.0.capacity {
            self.0.dropped.fetch_add(1, Ordering::Relaxed);
            return Err(Full { capacity: self.0.capacity });
        }
        queue.push_back(message);
        Ok(())
    }

    /// The oldest message.
    pub fn pop(&self) -> Option<Message> {
        self.0.queue.lock().unwrap_or_else(PoisonError::into_inner).pop_front()
    }

    /// How many messages wait.
    pub fn len(&self) -> usize {
        self.0.queue.lock().unwrap_or_else(PoisonError::into_inner).len()
    }

    /// Whether none wait.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many messages were dropped because the box was full, since it was made.
    pub fn dropped(&self) -> u64 {
        self.0.dropped.load(Ordering::Relaxed)
    }
}

/// What the courier tells whoever listens. None of it holds an address or a link.
#[non_exhaustive]
pub enum Note {
    /// A message was handed to the mailer.
    Sent,
    /// The mailer could not send a message. It is not tried again.
    Failed {
        /// The mailer's error.
        source: Box<dyn std::error::Error>,
    },
    /// Messages were dropped because the outbox was full.
    Dropped {
        /// How many in all.
        total: u64,
    },
}

impl fmt::Debug for Note {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sent => out.write_str("Sent"),
            Self::Failed { source } => write!(out, "Failed({source})"),
            Self::Dropped { total } => write!(out, "Dropped({total})"),
        }
    }
}

impl fmt::Display for Note {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sent => out.write_str("a sign-in message was sent"),
            Self::Failed { .. } => out.write_str("a sign-in message could not be sent"),
            Self::Dropped { total } => write!(out, "{total} sign-in messages were dropped: the outbox was full"),
        }
    }
}

/// Where the courier's notes go.
pub trait Notes {
    /// Takes one note.
    fn note(&self, note: &Note);
}

/// Sends what the outbox holds through a mailer, resting on the clock when it is empty.
#[derive(Debug)]
pub struct Courier<M, K> {
    outbox: Outbox,
    mailer: M,
    clock: K,
    rest: Duration,
    told: AtomicU64,
}

impl<M: Mailer, K: Clock> Courier<M, K> {
    /// A courier for `outbox` that rests `rest` when there is nothing to send.
    pub fn new(outbox: Outbox, mailer: M, clock: K, rest: Duration) -> Self {
        Self { outbox, mailer, clock, rest, told: AtomicU64::new(0) }
    }

    /// Sends everything waiting now, and reports what happened.
    pub async fn step<N: Notes>(&self, notes: &N) -> usize {
        let mut sent = 0;
        while let Some(message) = self.outbox.pop() {
            match self.mailer.send(&message).await {
                Ok(()) => {
                    sent += 1;
                    notes.note(&Note::Sent);
                }
                Err(error) => notes.note(&Note::Failed { source: Box::new(error) }),
            }
        }
        let total = self.outbox.dropped();
        if self.told.swap(total, Ordering::Relaxed) != total {
            notes.note(&Note::Dropped { total });
        }
        sent
    }

    /// Sends and rests until the clock says it has nothing more to give.
    pub async fn run<N: Notes>(&self, notes: &N) {
        loop {
            self.step(notes).await;
            if self.clock.sleep(self.rest).await == Slept::Stopped {
                break;
            }
        }
    }
}
