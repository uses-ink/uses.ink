//! A statement: a query built from typed values, never from text.

use sea_query::{
    DeleteStatement, IndexCreateStatement, InsertStatement, SelectStatement, TableCreateStatement, UpdateStatement,
};

/// What kind of statement it is, for saying which one failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Shape {
    /// A read.
    Select,
    /// A write that adds rows.
    Insert,
    /// A write that changes rows.
    Update,
    /// A write that removes rows.
    Delete,
    /// A table's definition.
    Table,
    /// An index's definition.
    Index,
}

impl std::fmt::Display for Shape {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Select => "a select",
            Self::Insert => "an insert",
            Self::Update => "an update",
            Self::Delete => "a delete",
            Self::Table => "a table definition",
            Self::Index => "an index definition",
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Query {
    Select(SelectStatement),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
    Table(TableCreateStatement),
    Index(IndexCreateStatement),
}

/// One statement, built with `sea_query`'s builders over typed identifiers. No text is written
/// by hand: a table is a type that names itself, a value is bound and never spliced, and the one
/// [`Renderer`](crate::Renderer) turns the statement into the dialect's text.
#[derive(Clone, Debug)]
pub struct Statement {
    query: Query,
}

impl Statement {
    /// D1 refuses a statement with more bound parameters than this, so every adapter does.
    pub const MAX_PARAMETERS: usize = 100;

    /// What kind of statement this is.
    pub const fn shape(&self) -> Shape {
        match self.query {
            Query::Select(_) => Shape::Select,
            Query::Insert(_) => Shape::Insert,
            Query::Update(_) => Shape::Update,
            Query::Delete(_) => Shape::Delete,
            Query::Table(_) => Shape::Table,
            Query::Index(_) => Shape::Index,
        }
    }

    pub(crate) const fn query(&self) -> &Query {
        &self.query
    }
}

macro_rules! statements {
    ($($builder:ident => $variant:ident),* $(,)?) => {$(
        impl From<$builder> for Statement {
            fn from(builder: $builder) -> Self {
                Self { query: Query::$variant(builder) }
            }
        }

        impl From<&mut $builder> for Statement {
            fn from(builder: &mut $builder) -> Self {
                Self { query: Query::$variant(builder.clone()) }
            }
        }
    )*};
}

statements! {
    SelectStatement => Select,
    InsertStatement => Insert,
    UpdateStatement => Update,
    DeleteStatement => Delete,
    TableCreateStatement => Table,
    IndexCreateStatement => Index,
}

impl logging::Kind for Shape {
    fn kind(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Insert => "insert",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Table => "table",
            Self::Index => "index",
        }
    }
}
