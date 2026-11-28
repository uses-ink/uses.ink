//! The conformance suite of the queue port: each case is a type with a `check` over any
//! [`Queue`], and [`suite!`] turns every case into a test. Time is a number of seconds the case
//! passes in, so no case sleeps.
//!
//! ```ignore
//! queue_conformance::suite!(|| (MyAdapter::in_memory(), ()));
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::assert_is_empty,
    clippy::missing_panics_doc,
    reason = "a conformance case stops at the first surprise"
)]

use std::time::Duration;

use model::Timestamp;
use queue::{Dedupe, Message, MessageId, Queue, Sent, Settled};

#[doc(hidden)]
pub use pollster;

/// Values the cases share.
struct Samples;

impl Samples {
    /// The clock of the suite: second `n` after an arbitrary start.
    fn at(n: i64) -> Timestamp {
        Timestamp::from_second(1_800_000_000 + n).unwrap()
    }

    /// A lease or delay of `n` seconds.
    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    /// A message with this body and no dedupe key.
    fn message(body: &str) -> Message {
        Message { body: body.as_bytes().to_vec(), dedupe: None }
    }

    /// A message with this body and key.
    fn keyed(body: &str, key: &str) -> Message {
        Message { body: body.as_bytes().to_vec(), dedupe: Some(key.parse::<Dedupe>().unwrap()) }
    }

    /// The id of a message that `sent` says was queued.
    fn queued(sent: Sent) -> MessageId {
        match sent {
            Sent::Queued(id) => id,
            other => panic!("expected a queued message, found {other:?}"),
        }
    }
}

/// Messages come out oldest first, once.
#[derive(Debug, Clone, Copy)]
pub struct Order;

impl Order {
    /// Three messages are received in the order they were sent, then none.
    pub async fn check<Q: Queue>(queue: &Q) {
        assert!(queue.receive(Samples::at(0), Samples::secs(30)).await.unwrap().is_none());
        for body in ["a", "b", "c"] {
            queue.send(Samples::message(body), Samples::at(0)).await.unwrap();
        }
        let mut seen = Vec::new();
        while let Some(delivery) = queue.receive(Samples::at(1), Samples::secs(30)).await.unwrap() {
            assert_eq!(delivery.attempt, 1);
            seen.push(String::from_utf8(delivery.body).unwrap());
        }
        assert_eq!(seen, ["a", "b", "c"]);
    }
}

/// A message is visible from the time it was sent.
#[derive(Debug, Clone, Copy)]
pub struct Visible;

impl Visible {
    /// Received before its time, a message is not there; at its time it is.
    pub async fn check<Q: Queue>(queue: &Q) {
        queue.send(Samples::message("later"), Samples::at(10)).await.unwrap();
        assert!(queue.receive(Samples::at(9), Samples::secs(30)).await.unwrap().is_none());
        assert_eq!(queue.receive(Samples::at(10), Samples::secs(30)).await.unwrap().unwrap().body, b"later");
    }
}

/// A message that is not acked comes back.
#[derive(Debug, Clone, Copy)]
pub struct Lease;

impl Lease {
    /// Hidden for the lease, then delivered again with the next attempt number.
    pub async fn check<Q: Queue>(queue: &Q) {
        let id = Samples::queued(queue.send(Samples::message("work"), Samples::at(0)).await.unwrap());
        let first = queue.receive(Samples::at(0), Samples::secs(30)).await.unwrap().unwrap();
        assert_eq!((first.id, first.attempt), (id, 1));
        assert!(
            queue.receive(Samples::at(29), Samples::secs(30)).await.unwrap().is_none(),
            "visible before the lease ended"
        );
        let second = queue.receive(Samples::at(30), Samples::secs(30)).await.unwrap().unwrap();
        assert_eq!((second.id, second.attempt, second.body.as_slice()), (id, 2, b"work".as_slice()));
        let third = queue.receive(Samples::at(60), Samples::secs(30)).await.unwrap().unwrap();
        assert_eq!(third.attempt, 3);
    }
}

/// Settling a delivery.
#[derive(Debug, Clone, Copy)]
pub struct Acks;

impl Acks {
    /// An ack removes the message for good, once; a stale delivery settles nothing.
    pub async fn check<Q: Queue>(queue: &Q) {
        queue.send(Samples::message("work"), Samples::at(0)).await.unwrap();
        let first = queue.receive(Samples::at(0), Samples::secs(30)).await.unwrap().unwrap();
        // the lease ends and a second consumer takes the message
        let second = queue.receive(Samples::at(31), Samples::secs(30)).await.unwrap().unwrap();
        assert_eq!(queue.ack(&first).await.unwrap(), Settled::Stale, "a slow consumer acked another's message");
        assert_eq!(queue.release(&first, Samples::at(31), Samples::secs(0)).await.unwrap(), Settled::Stale);
        assert_eq!(queue.ack(&second).await.unwrap(), Settled::Done);
        assert_eq!(queue.ack(&second).await.unwrap(), Settled::Stale);
        assert!(queue.receive(Samples::at(1000), Samples::secs(30)).await.unwrap().is_none());
    }
}

/// Giving a message back.
#[derive(Debug, Clone, Copy)]
pub struct Release;

impl Release {
    /// A released message is hidden for the delay, then delivered again, its attempts kept.
    pub async fn check<Q: Queue>(queue: &Q) {
        queue.send(Samples::message("retry"), Samples::at(0)).await.unwrap();
        let first = queue.receive(Samples::at(0), Samples::secs(300)).await.unwrap().unwrap();
        assert_eq!(queue.release(&first, Samples::at(5), Samples::secs(10)).await.unwrap(), Settled::Done);
        assert!(queue.receive(Samples::at(14), Samples::secs(300)).await.unwrap().is_none());
        let again = queue.receive(Samples::at(15), Samples::secs(300)).await.unwrap().unwrap();
        assert_eq!((again.id, again.attempt), (first.id, 2));
    }
}

/// The dedupe key.
#[derive(Debug, Clone, Copy)]
pub struct Dedupes;

impl Dedupes {
    /// A key is held while its message is in the queue, leased or not, and free once it is acked;
    /// messages with no key, or another key, are never dropped.
    pub async fn check<Q: Queue>(queue: &Q) {
        let id = Samples::queued(queue.send(Samples::keyed("push", "delivery-1"), Samples::at(0)).await.unwrap());
        assert_eq!(
            queue.send(Samples::keyed("push again", "delivery-1"), Samples::at(1)).await.unwrap(),
            Sent::Duplicate(id)
        );
        queue.send(Samples::keyed("other", "delivery-2"), Samples::at(1)).await.unwrap();
        queue.send(Samples::message("keyless"), Samples::at(1)).await.unwrap();
        queue.send(Samples::message("keyless"), Samples::at(1)).await.unwrap();
        let first = queue.receive(Samples::at(2), Samples::secs(30)).await.unwrap().unwrap();
        assert_eq!(
            (first.id, first.body.as_slice()),
            (id, b"push".as_slice()),
            "the duplicate replaced the first message"
        );
        assert_eq!(
            queue.send(Samples::keyed("push again", "delivery-1"), Samples::at(3)).await.unwrap(),
            Sent::Duplicate(id),
            "a leased message holds its key"
        );
        queue.ack(&first).await.unwrap();
        assert!(matches!(
            queue.send(Samples::keyed("push again", "delivery-1"), Samples::at(4)).await.unwrap(),
            Sent::Queued(_)
        ));
        let mut rest = 0;
        while queue.receive(Samples::at(5), Samples::secs(30)).await.unwrap().is_some() {
            rest += 1;
        }
        assert_eq!(rest, 4, "one message was dropped that should not have been, or one was kept that should not");
    }
}

/// Bodies are bytes.
#[derive(Debug, Clone, Copy)]
pub struct Bodies;

impl Bodies {
    /// Any bytes, including none, come out as they went in.
    pub async fn check<Q: Queue>(queue: &Q) {
        let bodies = [Vec::new(), vec![0, 255, 0], "h\u{e9}llo".as_bytes().to_vec(), vec![9; 100_000]];
        for body in &bodies {
            queue.send(Message { body: body.clone(), dedupe: None }, Samples::at(0)).await.unwrap();
        }
        for body in &bodies {
            assert_eq!(&queue.receive(Samples::at(1), Samples::secs(30)).await.unwrap().unwrap().body, body);
        }
    }
}

/// Runs every case of the queue port's suite against the adapter `$make` builds: a closure that
/// returns the adapter, fresh and empty, and whatever must outlive it (or `()`) as a pair. Each
/// case is a test of its own, so each has its own budget.
#[macro_export]
macro_rules! suite {
    ($make:expr) => {
        $crate::suite!(@case $make, delivers_oldest_first_and_once, Order);
        $crate::suite!(@case $make, hides_a_message_until_its_time, Visible);
        $crate::suite!(@case $make, delivers_again_when_a_lease_ends, Lease);
        $crate::suite!(@case $make, settles_only_a_current_delivery, Acks);
        $crate::suite!(@case $make, releases_a_message_for_a_delay, Release);
        $crate::suite!(@case $make, drops_a_message_whose_key_is_held, Dedupes);
        $crate::suite!(@case $make, carries_any_bytes, Bodies);
    };
    (@case $make:expr, $name:ident, $case:ident) => {
        #[test]
        fn $name() {
            let (queue, _guard) = ($make)();
            $crate::pollster::block_on($crate::$case::check(&queue));
        }
    };
}
