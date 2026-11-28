//! The adapter against the port's conformance suite.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

extern crate uses_ink_db_sqlite as db_sqlite;

db_conformance::suite!(|| (db_sqlite::Sqlite::memory().unwrap(), ()));
