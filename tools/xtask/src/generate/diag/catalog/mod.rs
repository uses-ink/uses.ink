//! `diag/catalog.toml`, read and checked. Every flaw in the file is reported at once.

mod field;
mod flaw;
mod read;
mod schema;
mod template;
#[cfg(test)]
mod test;

pub(crate) use field::{Field, Kind};
pub(crate) use flaw::Flaw;
pub(crate) use schema::*;
pub(crate) use template::{Part, Piece, Template};
