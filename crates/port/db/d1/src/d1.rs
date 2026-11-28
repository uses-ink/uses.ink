//! The adapter.

use db::{Db, Outcome, Renderer, Row, Shape, Statement};
use logging::Field as _;
use worker::{D1Database, D1PreparedStatement, D1Result};

use crate::cell::{Bound, Cell, Record};
use crate::error::{Error, Remote, Stage};

/// A D1 database: a binding of a Worker.
#[derive(Debug)]
pub struct D1 {
    database: D1Database,
}

impl D1 {
    /// The database a Worker's D1 binding gives, from `env.d1(..)`.
    pub fn new(database: D1Database) -> Self {
        Self { database }
    }

    /// `statement` rendered and bound, ready to run.
    fn prepare(&self, statement: &Statement, index: usize) -> Result<D1PreparedStatement, Error> {
        let rendered = Renderer.render(statement).map_err(|source| Error::Render { index, source })?;
        let mut values = Vec::with_capacity(rendered.values().len());
        for value in rendered.values() {
            values.push(Bound::js(value).map_err(|source| Error::Render { index, source: source.into() })?);
        }
        let stage = Stage::Statement(statement.shape());
        self.database
            .prepare(rendered.text())
            .bind(&values)
            .map_err(|source| Error::Run { stage, source: Remote::from(source) })
    }

    /// The row `cells` make, named `names`.
    fn row(names: &[String], cells: Vec<Cell>) -> Result<Row, Error> {
        let mut columns = Vec::with_capacity(cells.len());
        for (name, cell) in names.iter().zip(cells) {
            let value = cell.value().map_err(|source| Error::Read { column: name.clone(), source })?;
            columns.push((name.clone(), value));
        }
        Ok(Row::new(columns))
    }

    /// What D1 answered to one statement of `shape`: its rows by name, and what it changed.
    fn outcome(result: &D1Result, shape: Shape) -> Result<Outcome, Error> {
        let stage = Stage::Statement(shape);
        let records = result.results::<Record>().map_err(|source| Error::Run { stage, source: source.into() })?;
        let mut rows = Vec::with_capacity(records.len());
        for Record(entries) in records {
            let (names, cells): (Vec<String>, Vec<Cell>) = entries.into_iter().unzip();
            rows.push(Self::row(&names, cells)?);
        }
        let changes = if shape == Shape::Select {
            0
        } else {
            let meta = result.meta().map_err(|source| Error::Run { stage, source: source.into() })?;
            let changed = meta.and_then(|meta| meta.changes).ok_or(Error::Meta { stage })?;
            u64::try_from(changed).map_err(|_| Error::Meta { stage })?
        };
        Ok(Outcome { rows, changes })
    }

    /// Runs one statement inside its `statement` span.
    async fn execute(&self, statement: &Statement) -> Result<Outcome, Error> {
        let span = logging::statement!(statement.shape());
        let outcome = self.exec(statement).await;
        span.in_scope(|| match &outcome {
            Ok(done) => logging::debug!(
                name: logging::event::DB_STATEMENT,
                rows = logging::Count::from(done.rows.len()).field(),
                changes = logging::Count(done.changes).field(),
            ),
            Err(error) => logging::warn!(name: logging::event::DB_STATEMENT, error = logging::Failure(error).field()),
        });
        outcome
    }

    async fn exec(&self, statement: &Statement) -> Result<Outcome, Error> {
        let prepared = self.prepare(statement, 0)?;
        let shape = statement.shape();
        let stage = Stage::Statement(shape);
        if shape == Shape::Select {
            // the column names come with the rows, in order, one for each column even when two
            // share a name
            let (names, found) = prepared
                .raw_with_column_names::<Cell>()
                .await
                .map_err(|source| Error::Run { stage, source: source.into() })?;
            let rows = found.into_iter().map(|cells| Self::row(&names, cells)).collect::<Result<Vec<_>, _>>()?;
            return Ok(Outcome { rows, changes: 0 });
        }
        let result = prepared.run().await.map_err(|source| Error::Run { stage, source: source.into() })?;
        Self::outcome(&result, shape)
    }
}

impl Db for D1 {
    type Error = Error;

    async fn run(&self, statement: Statement) -> Result<Outcome, db::Error<Error>> {
        Ok(self.execute(&statement).await?)
    }

    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<Outcome>, db::Error<Error>> {
        // D1 refuses an empty batch
        if statements.is_empty() {
            return Ok(Vec::new());
        }
        let prepared = statements
            .iter()
            .enumerate()
            .map(|(index, statement)| self.prepare(statement, index))
            .collect::<Result<Vec<_>, _>>()?;
        let results = self
            .database
            .batch(prepared)
            .await
            .map_err(|source| Error::Run { stage: Stage::Batch, source: source.into() })?;
        if results.len() != statements.len() {
            return Err(Error::Meta { stage: Stage::Batch }.into());
        }
        let outcomes = results
            .iter()
            .zip(&statements)
            .map(|(result, statement)| Self::outcome(result, statement.shape()))
            .collect::<Result<Vec<_>, _>>()?;
        logging::debug!(name: logging::event::DB_BATCH, statements = logging::Count::from(outcomes.len()).field());
        Ok(outcomes)
    }
}
