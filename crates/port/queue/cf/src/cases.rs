//! The cases of the wake-up, beside the port's conformance suite: public so the Worker rig runs
//! the same ones in workerd.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::assert_is_empty,
    clippy::missing_panics_doc,
    reason = "a conformance case stops at the first surprise"
)]

use std::time::Duration;

use db::Db;
use model::Timestamp;
use queue::{Message, Queue, Sent, Settled};
use queue_rows::Local;

use crate::cf::Cf;
use crate::recorder::Recorder;
use crate::wake::Nudge;

/// Values the cases share.
struct Samples;

impl Samples {
    /// The clock of the cases: second `n` after an arbitrary start.
    fn at(n: i64) -> Timestamp {
        Timestamp::from_second(1_800_000_000 + n).unwrap()
    }

    /// A message with this body and key.
    fn keyed(body: &str, key: &str) -> Message {
        Message { body: body.as_bytes().to_vec(), dedupe: Some(key.parse().unwrap()) }
    }
}

/// A wake-up follows what made a message visible, and only that.
#[derive(Debug, Clone, Copy)]
pub struct Nudged;

impl Nudged {
    /// A new message nudges once, at no delay; a duplicate nudges not at all; a release for a delay
    /// nudges at that delay; a stale release and an ack do not. `db` is empty and has the queue's
    /// table.
    pub async fn check<D: Db>(db: D) {
        let queue = Cf::new(Local::new(db), Recorder::default());
        let first = queue.send(Samples::keyed("one", "k"), Samples::at(0)).await.unwrap();
        assert!(matches!(first, Sent::Queued(_)));
        assert_eq!(queue.wake().nudges().iter().map(Nudge::seconds).collect::<Vec<_>>(), [0]);
        let again = queue.send(Samples::keyed("two", "k"), Samples::at(0)).await.unwrap();
        assert!(matches!(again, Sent::Duplicate(_)));
        assert_eq!(queue.wake().nudges().len(), 1, "a duplicate is not woken for");
        let delivery = queue.receive(Samples::at(1), Duration::from_secs(30)).await.unwrap().unwrap();
        assert_eq!(queue.wake().nudges().len(), 1, "a receive wakes nothing");
        let released = queue.release(&delivery, Samples::at(2), Duration::from_secs(90)).await.unwrap();
        assert_eq!(released, Settled::Done);
        assert_eq!(queue.wake().nudges().last().map(Nudge::seconds), Some(90));
        let second = queue.receive(Samples::at(100), Duration::from_secs(30)).await.unwrap().unwrap();
        let stale = queue.release(&delivery, Samples::at(101), Duration::from_secs(5)).await.unwrap();
        assert_eq!(stale, Settled::Stale);
        assert_eq!(queue.wake().nudges().len(), 2, "a stale release wakes nothing");
        assert_eq!(queue.ack(&second).await.unwrap(), Settled::Done);
        assert_eq!(queue.wake().nudges().len(), 2, "an ack wakes nothing");
    }
}

/// A wake-up that fails is not the answer.
#[derive(Debug, Clone, Copy)]
pub struct Refused;

impl Refused {
    /// With every wake-up refused, `send` still answers `Queued` and the message is received, and
    /// `release` still answers `Done`. `db` is empty and has the queue's table.
    pub async fn check<D: Db>(db: D) {
        let queue = Cf::new(Local::new(db), Recorder::default());
        queue.wake().refuse(true);
        let sent = queue.send(Samples::keyed("one", "k"), Samples::at(0)).await.unwrap();
        assert!(matches!(sent, Sent::Queued(_)), "{sent:?}");
        assert!(queue.wake().nudges().is_empty());
        let delivery = queue.receive(Samples::at(1), Duration::from_secs(30)).await.unwrap().unwrap();
        assert_eq!(delivery.body, b"one");
        let released = queue.release(&delivery, Samples::at(2), Duration::from_secs(10)).await.unwrap();
        assert_eq!(released, Settled::Done);
        queue.wake().refuse(false);
        let received = queue.receive(Samples::at(20), Duration::from_secs(30)).await.unwrap().unwrap();
        assert_eq!(received.attempt, 2, "the rows kept the message and its count");
    }
}
