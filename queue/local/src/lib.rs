//! The queue port in the rows of any [`db::Db`] (decision 0045).
//!
//! Self-hosted, the queue is a table in the same SQLite file as everything else, so a build
//! request needs no second connection, no second file and no second thing to back up. The claim
//! is one statement, an update of the first visible message that returns it, so two consumers
//! never take the same message, and it needs no interactive transaction, so the adapter also
//! runs on D1. A message that is not acked is delivered again when its lease ends.
//!
//! The table is made by the [`Queued`] migration, which the host applies with the control
//! plane's own.

mod error;
mod local;
mod schema;

pub use error::Error;
pub use local::Local;
pub use schema::Queued;
