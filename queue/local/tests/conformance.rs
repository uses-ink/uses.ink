//! The adapter, over SQLite in memory, against the port's conformance suite.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_queue_local as queue_local;

use db::Steps;
use db_sqlite::Sqlite;
use queue_local::{Local, Queued};

/// A queue in a new database with its table made.
fn fresh() -> (Local<Sqlite>, ()) {
    let db = Sqlite::memory().unwrap();
    pollster::block_on(Steps::new().with::<Queued>().unwrap().apply(&db)).unwrap();
    (Local::new(db), ())
}

queue_conformance::suite!(fresh);
