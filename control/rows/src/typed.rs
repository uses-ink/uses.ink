//! The kinds of column most of a schema is made of.

use sea_query::{ColumnDef, IntoIden};

/// The two kinds of column most of the schema is made of.
pub struct Typed;

impl Typed {
    /// A text column that cannot be null.
    pub fn text<C: IntoIden>(column: C) -> ColumnDef {
        ColumnDef::new(column).text().not_null().to_owned()
    }

    /// A column of unix milliseconds that cannot be null.
    pub fn time<C: IntoIden>(column: C) -> ColumnDef {
        ColumnDef::new(column).big_integer().not_null().to_owned()
    }
}
