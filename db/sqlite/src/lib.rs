//! The row port on SQLite, through `rusqlite` with the C library bundled (decision 0045).
//!
//! One connection behind a mutex: self-hosting has one process, and D1, the stricter target,
//! has no interactive transaction either. A batch is one SQLite transaction. Foreign keys are
//! enforced, as they are on D1, and a file database runs in WAL mode.

mod error;
mod sqlite;

pub use error::Error;
pub use sqlite::Sqlite;
