//! `diag/catalog.toml` into `model/src/generated.rs` (the `Problem` and `Announcement` enums,
//! their codes, severities and faults, and their coded form) and `diag/src/generated.rs` (their English wording).

mod catalog;
mod rust;

use crate::Task;
use crate::error::Error;
use crate::generate::output::{Files, Generator};

pub(crate) use catalog::Flaw;

/// `cargo xtask diag`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Diag;

impl Diag {
    const MODEL: &'static str = "model/src/generated.rs";
    const WORDING: &'static str = "diag/src/generated.rs";
}

impl Generator for Diag {
    const TASK: Task = Task::Diag;
    const SOURCE: &'static str = "diag/catalog.toml";
    type Source = catalog::Catalog;

    fn generate(&self, catalog: &catalog::Catalog, files: &mut Files<'_>) -> Result<(), Error> {
        files.rust(Self::MODEL, catalog.model())?;
        files.rust(Self::WORDING, catalog.wording())
    }
}

impl From<Vec<Flaw>> for crate::error::Flaw {
    fn from(flaws: Vec<Flaw>) -> Self {
        Self::Catalog(flaws)
    }
}
