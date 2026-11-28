//! The row port of the control plane, shaped like Cloudflare D1 (decision 0030).
//!
//! - [`Statement`]: a query built from `sea_query`'s typed builders over identifier types, with
//!   its values bound ([`Value`]); no SQL text is written by hand, and [`Renderer`] is the one
//!   place a statement becomes text.
//! - [`Db`]: run one statement, or an atomic batch of them. No interactive transaction exists,
//!   because D1 has none; a change that must be atomic is one batch.
//! - [`Row`]: what a statement returns, read back as typed columns ([`Column`]).
//! - [`Migration`] and [`Steps`]: migrations as typed values with a number and a name, and the
//!   runner that applies the missing ones through any [`Db`], writing wrangler's own
//!   `d1_migrations` ledger.
//!
//! The adapters are `db/sqlite` and, hosted, `db/d1`; the conformance suite every adapter runs
//! is `db/conformance`.

mod error;
mod port;
mod render;
mod row;
mod statement;
mod step;
mod value;

pub use error::{Constraint, Error};
pub use port::{Db, Outcome};
pub use render::{Rendered, Renderer, TooMany, Unrenderable, Unsupported};
pub use row::{Column, ColumnError, ColumnFault, Row};
pub use statement::{Shape, Statement};
pub use step::{Applied, Ledger, Migration, Script, StepError, Steps, Unapplied};

/// The builders statements are made with, so every crate shares one version of them.
pub use sea_query;
pub use value::{Kind, Mismatch, Value};
