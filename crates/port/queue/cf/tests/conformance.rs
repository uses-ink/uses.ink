//! The adapter over SQLite in memory, against the port's conformance suite and its own cases.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_queue_cf as queue_cf;

use db::Steps;
use db_sqlite::Sqlite;
use queue_cf::{Cf, Nudged, Recorder, Refused};
use queue_rows::{Local, Queued};

/// A database with the queue's table made.
fn migrated() -> Sqlite {
    let db = Sqlite::memory().unwrap();
    pollster::block_on(Steps::new().with::<Queued>().unwrap().apply(&db)).unwrap();
    db
}

/// A queue in a new database, its wake-up recorded.
fn fresh() -> (Cf<Sqlite, Recorder>, ()) {
    (Cf::new(Local::new(migrated()), Recorder::default()), ())
}

queue_conformance::suite!(fresh);

#[test]
fn nudges_after_what_makes_a_message_visible() {
    pollster::block_on(Nudged::check(migrated()));
}

#[test]
fn keeps_its_answers_when_the_wake_up_fails() {
    pollster::block_on(Refused::check(migrated()));
}
