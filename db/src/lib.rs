//! The row port of the control plane, shaped like Cloudflare D1 (decision 0030).
//!
//! - [`Statement`]: raw SQL that is `&'static str`, so a statement can never be built from
//!   input, and the values bound to it ([`Value`]).
//! - [`Db`]: run one statement, or an atomic batch of them. No interactive transaction exists,
//!   because D1 has none; a change that must be atomic is one batch.
//! - [`Row`]: what a statement returns, read back as typed columns ([`Column`]).
//! - [`Script`] and [`Steps`]: a migration folder of plain `.sql` files in wrangler's format,
//!   and the runner that applies the missing ones through any [`Db`], writing wrangler's own
//!   `d1_migrations` ledger.
//!
//! The adapters are `db/sqlite` and, hosted, `db/d1`; the conformance suite every adapter runs
//! is `db/conformance`.

mod error;
mod port;
mod row;
mod script;
mod statement;
mod step;
mod value;

pub use error::{Constraint, Error};
pub use port::{Db, Outcome};
pub use row::{Column, ColumnError, ColumnFault, Row};
pub use script::{Keyword, Script, ScriptError};
pub use statement::{Statement, TooMany};
pub use step::{Applied, Step, StepError, Steps, Unapplied};
pub use value::{Kind, Mismatch, Value};
