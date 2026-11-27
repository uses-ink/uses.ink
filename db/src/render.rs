//! The one place a statement becomes text.

use std::fmt;

use sea_query::{SchemaStatementBuilder, SqliteQueryBuilder};

use crate::statement::Query;
use crate::{Statement, Value};

/// A statement as the dialect writes it: the text, with `?` for each value, and the values in
/// the order of the `?`.
#[derive(Clone, Debug, PartialEq)]
pub struct Rendered {
    text: String,
    values: Vec<Value>,
}

impl Rendered {
    /// The text. It holds identifiers and keywords only; every value is in [`Rendered::values`].
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The bound values, in order.
    pub fn values(&self) -> &[Value] {
        &self.values
    }
}

/// Turns a [`Statement`] into [`Rendered`], in SQLite's dialect, which D1 shares. It is the only
/// code in the workspace that touches SQL text, and it checks the parameter limit of D1.
#[derive(Clone, Copy, Debug, Default)]
pub struct Renderer;

impl Renderer {
    /// Renders `statement`, or says why it cannot be run.
    pub fn render(&self, statement: &Statement) -> Result<Rendered, Unrenderable> {
        let (text, values) = match statement.query() {
            Query::Select(query) => query.build(SqliteQueryBuilder),
            Query::Insert(query) => query.build(SqliteQueryBuilder),
            Query::Update(query) => query.build(SqliteQueryBuilder),
            Query::Delete(query) => query.build(SqliteQueryBuilder),
            Query::Table(query) => (SchemaStatementBuilder::build(query, SqliteQueryBuilder), sea_query::Values(Vec::new())),
            Query::Index(query) => (SchemaStatementBuilder::build(query, SqliteQueryBuilder), sea_query::Values(Vec::new())),
        };
        if values.0.len() > Statement::MAX_PARAMETERS {
            return Err(Unrenderable::TooMany(TooMany { count: values.0.len(), limit: Statement::MAX_PARAMETERS }));
        }
        let values = values.0.into_iter().map(Value::try_from).collect::<Result<Vec<_>, _>>()?;
        Ok(Rendered { text, values })
    }
}

/// A statement bound with more parameters than the port allows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooMany {
    /// How many were bound.
    pub count: usize,
    /// How many are allowed.
    pub limit: usize,
}

impl fmt::Display for TooMany {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{} parameters are bound and the limit is {}", self.count, self.limit)
    }
}

impl std::error::Error for TooMany {}

/// A bound value D1 cannot carry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unsupported {
    /// The kind of value `sea_query` held.
    pub kind: &'static str,
}

impl fmt::Display for Unsupported {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "a bound value of type `{}` cannot be stored", self.kind)
    }
}

impl std::error::Error for Unsupported {}

/// A statement that cannot be run, whatever the database.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unrenderable {
    /// More parameters than D1 accepts.
    TooMany(TooMany),
    /// A value of a type the port does not carry.
    Value(Unsupported),
}

impl fmt::Display for Unrenderable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooMany(source) => source.fmt(out),
            Self::Value(source) => source.fmt(out),
        }
    }
}

impl std::error::Error for Unrenderable {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TooMany(source) => Some(source),
            Self::Value(source) => Some(source),
        }
    }
}

impl From<Unsupported> for Unrenderable {
    fn from(source: Unsupported) -> Self {
        Self::Value(source)
    }
}
