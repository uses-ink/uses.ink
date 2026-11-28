//! The control plane's migrations in the order a Worker's database has them.

use control::{Domain, Remembered, Schema, Treed};
use db::{StepError, Steps};
use queue_rows::Queued;

/// The migrations a host applies, once: the same list as `ink serve`'s, so a D1 database and a
/// SQLite file end with the same schema.
#[derive(Clone, Copy, Debug)]
pub struct Migrations;

impl Migrations {
    /// Every migration, in order.
    pub fn steps() -> Result<Steps, StepError> {
        Schema::after(Steps::new().with::<Domain>()?.with::<Remembered>()?.with::<Treed>()?.with::<Queued>()?)
    }
}
