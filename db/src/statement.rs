//! A statement: SQL fixed at compile time and the values bound to it.

use std::fmt;

use crate::Value;

/// One prepared statement. Its SQL is `&'static str`, so no value from outside can become SQL:
/// the only way in is [`Statement::bind`].
#[derive(Clone, Debug, PartialEq)]
pub struct Statement {
    sql: &'static str,
    parameters: Vec<Value>,
}

impl Statement {
    /// D1 refuses a statement with more bound parameters than this, so every adapter does.
    pub const MAX_PARAMETERS: usize = 100;

    /// A statement of `sql`, with nothing bound yet. Parameters are `?` and bind in order.
    pub const fn prepare(sql: &'static str) -> Self {
        Self { sql, parameters: Vec::new() }
    }

    /// Binds the next `?`.
    #[must_use]
    pub fn bind(mut self, value: impl Into<Value>) -> Self {
        self.parameters.push(value.into());
        self
    }

    /// The SQL.
    pub const fn sql(&self) -> &'static str {
        self.sql
    }

    /// The bound values, in order.
    pub fn parameters(&self) -> &[Value] {
        &self.parameters
    }

    /// Refuses more than [`Statement::MAX_PARAMETERS`] bound values.
    pub fn check(&self) -> Result<(), TooMany> {
        match self.parameters.len() {
            count if count > Self::MAX_PARAMETERS => Err(TooMany { count, limit: Self::MAX_PARAMETERS }),
            _ => Ok(()),
        }
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
