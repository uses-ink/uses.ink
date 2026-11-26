//! `diag/catalog.toml` into `model/src/generated.rs` (the `Problem` and `Announcement` enums,
//! their codes, severities and faults) and `diag/src/generated.rs` (their English wording).

mod catalog;
mod rust;

use crate::error::Error;
use crate::output::Output;

pub(crate) use catalog::Flaw;

const SOURCE: &str = "diag/catalog.toml";
const MODEL: &str = "model/src/generated.rs";
const WORDING: &str = "diag/src/generated.rs";

pub(crate) fn write(output: &mut Output) -> Result<(), Error> {
    let text = output.read(SOURCE)?;
    let table: toml::Table = toml::from_str(&text).map_err(|source| Error::Toml { path: SOURCE.into(), source })?;
    let catalog = catalog::Catalog::parse(table).map_err(Error::Catalog)?;
    output.put_rust(MODEL, SOURCE, "diag", rust::model(&catalog))?;
    output.put_rust(WORDING, SOURCE, "diag", rust::wording(&catalog))
}
