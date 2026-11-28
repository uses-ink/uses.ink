//! The hold table, as a migration after the recovery codes' and the sign-in's.

use db::{Migration, Statement};
use sea_query::{ColumnDef, Table};

use super::Why;
use crate::table::{Allowed, Hold};
use crate::typed::Typed;

/// The migration that makes `hold`: a row per handle that is held after its site left it. A
/// tombstone made before this migration has no row here and is held from the day it was made
/// for as long as the operator's hold is (`Control::held`).
#[derive(Clone, Copy, Debug)]
pub struct Held;

impl Migration for Held {
    const NUMBER: u32 = 301;
    const NAME: &'static str = "hold";

    fn statements() -> Vec<Statement> {
        vec![
            Table::create()
                .table(Hold::Table)
                .col(ColumnDef::new(Hold::Handle).text().not_null().primary_key())
                .col(Typed::text(Hold::SiteId))
                .col(ColumnDef::new(Hold::Why).text().not_null().check(Allowed::of(Hold::Why, &Why::NAMES)))
                // null is for good
                .col(ColumnDef::new(Hold::Until).big_integer())
                .col(Typed::time(Hold::At))
                .into(),
        ]
    }
}
