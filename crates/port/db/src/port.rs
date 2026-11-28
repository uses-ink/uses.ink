//! The port itself.

use crate::{Error, Row, Statement};

/// What one statement did: the rows it returned and the rows it changed.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Outcome {
    /// The rows a `SELECT` or `RETURNING` produced, in order.
    pub rows: Vec<Row>,
    /// How many rows an `INSERT`, `UPDATE` or `DELETE` changed.
    pub changes: u64,
}

/// A database shaped like D1: one statement, or an atomic batch, and nothing in between.
///
/// A change that must be atomic is one [`Db::batch`]; a change that depends on what was read is
/// a single statement with its condition in `WHERE`, checked by [`Outcome::changes`]. Futures are
/// not required to be `Send`: D1's are not.
#[expect(async_fn_in_trait, reason = "D1 and R2 futures are not Send on Workers, so no Send bound can be promised")]
pub trait Db {
    /// The adapter's own error.
    type Error: std::error::Error + 'static;

    /// Runs one statement.
    async fn run(&self, statement: Statement) -> Result<Outcome, Error<Self::Error>>;

    /// Runs the statements in order as one atomic unit: if one fails, none is applied. The
    /// outcomes are in the statements' order.
    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<Outcome>, Error<Self::Error>>;

    /// The rows a statement returns.
    async fn rows(&self, statement: Statement) -> Result<Vec<Row>, Error<Self::Error>> {
        Ok(self.run(statement).await?.rows)
    }

    /// The first row a statement returns, if any.
    async fn first(&self, statement: Statement) -> Result<Option<Row>, Error<Self::Error>> {
        Ok(self.run(statement).await?.rows.into_iter().next())
    }
}

impl<D: Db + ?Sized> Db for &D {
    type Error = D::Error;

    async fn run(&self, statement: Statement) -> Result<Outcome, Error<Self::Error>> {
        (**self).run(statement).await
    }

    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<Outcome>, Error<Self::Error>> {
        (**self).batch(statements).await
    }
}

impl<D: Db + ?Sized> Db for std::sync::Arc<D> {
    type Error = D::Error;

    async fn run(&self, statement: Statement) -> Result<Outcome, Error<Self::Error>> {
        (**self).run(statement).await
    }

    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<Outcome>, Error<Self::Error>> {
        (**self).batch(statements).await
    }
}
