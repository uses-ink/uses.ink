//! `diag/catalog.toml`, read and checked.

mod field;
mod schema;
#[cfg(test)]
mod test;

pub(crate) use field::{Field, Kind};
pub(crate) use schema::*;
