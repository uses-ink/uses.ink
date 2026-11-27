//! The adapter.

use std::fmt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use db::{Constraint, Db, Outcome, Renderer, Row, Statement, Value};
use rusqlite::types::{Value as Stored, ValueRef};
use rusqlite::{Connection, ffi};

use trace::Field as _;

use crate::{Error, Stage};

/// A SQLite database: a file, or memory.
pub struct Sqlite {
    connection: Mutex<Connection>,
    path: Option<std::path::PathBuf>,
}

impl fmt::Debug for Sqlite {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("Sqlite").field("path", &self.path).finish_non_exhaustive()
    }
}

impl Sqlite {
    /// How long a statement waits for another process's lock before it fails.
    const BUSY: std::time::Duration = std::time::Duration::from_secs(5);

    /// A database that lives as long as this value.
    pub fn memory() -> Result<Self, Error> {
        let connection = Connection::open_in_memory().map_err(|source| Error::Open { path: None, source })?;
        Self::set_up(connection, None)
    }

    /// The database in `path`, created when it is missing, in WAL mode.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let open = |source| Error::Open { path: Some(path.to_path_buf()), source };
        let connection = Connection::open(path).map_err(open)?;
        connection.pragma_update(None, "journal_mode", "WAL").map_err(open)?;
        connection.pragma_update(None, "synchronous", "NORMAL").map_err(open)?;
        Self::set_up(connection, Some(path.to_path_buf()))
    }

    fn set_up(connection: Connection, path: Option<std::path::PathBuf>) -> Result<Self, Error> {
        let open = |source| Error::Open { path: path.clone(), source };
        // D1 enforces foreign keys, so the adapter does too
        connection.pragma_update(None, "foreign_keys", true).map_err(open)?;
        connection.busy_timeout(Self::BUSY).map_err(open)?;
        Ok(Self { connection: Mutex::new(connection), path })
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, Error> {
        self.connection.lock().map_err(|_| Error::Poisoned)
    }
}

/// What one statement does on a connection.
struct Execution<'a> {
    connection: &'a Connection,
}

impl Execution<'_> {
    fn stored(value: &Value) -> Result<Stored, db::Unsupported> {
        Ok(match value {
            Value::Null => Stored::Null,
            Value::Integer(n) => Stored::Integer(*n),
            Value::Real(n) => Stored::Real(*n),
            Value::Text(text) => Stored::Text(text.clone()),
            Value::Blob(bytes) => Stored::Blob(bytes.clone()),
            other => return Err(db::Unsupported::Kind { kind: other.kind() }),
        })
    }

    fn value(name: &str, value: ValueRef<'_>) -> Result<Value, Error> {
        Ok(match value {
            ValueRef::Null => Value::Null,
            ValueRef::Integer(n) => Value::Integer(n),
            ValueRef::Real(n) => Value::Real(n),
            ValueRef::Text(bytes) => Value::Text(
                std::str::from_utf8(bytes)
                    .map_err(|source| Error::Text { column: name.to_owned(), source })?
                    .to_owned(),
            ),
            ValueRef::Blob(bytes) => Value::Blob(bytes.to_vec()),
        })
    }

    /// Runs one statement inside its `statement` span: its shape, how many rows came back and
    /// how many it changed. Never its text and never a value bound to it.
    fn run(&self, statement: &Statement, index: usize) -> Result<Outcome, Error> {
        let span = trace::statement!(statement.shape());
        let outcome = span.in_scope(|| self.execute(statement, index));
        span.in_scope(|| match &outcome {
            Ok(done) => trace::debug!(
                name: trace::event::DB_STATEMENT,
                rows = trace::Count::from(done.rows.len()).field(),
                changes = trace::Count(done.changes).field(),
            ),
            Err(error) => trace::warn!(name: trace::event::DB_STATEMENT, error = trace::Failure(error).field()),
        });
        outcome
    }

    fn execute(&self, statement: &Statement, index: usize) -> Result<Outcome, Error> {
        let rendered = Renderer.render(statement).map_err(|source| Error::Render { index, source })?;
        let failed = |source| Error::Run { stage: Stage::Statement(statement.shape()), source };
        let mut prepared = self.connection.prepare_cached(rendered.text()).map_err(failed)?;
        let readonly = prepared.readonly();
        let names: Vec<String> = prepared.column_names().map(str::to_owned).collect();
        let mut stored = Vec::with_capacity(rendered.values().len());
        for value in rendered.values() {
            stored.push(Self::stored(value).map_err(|source| Error::Render { index, source: source.into() })?);
        }
        let bound = rusqlite::params_from_iter(stored);
        let mut rows = prepared.query(bound).map_err(failed)?;
        let mut found = Vec::new();
        while let Some(row) = rows.next().map_err(failed)? {
            let mut columns = Vec::with_capacity(names.len());
            for (at, name) in names.iter().enumerate() {
                let value = row.get_ref(at).map_err(failed)?;
                columns.push((name.clone(), Self::value(name, value)?));
            }
            found.push(Row::new(columns));
        }
        drop(rows);
        drop(prepared);
        let changes = if readonly { 0 } else { self.connection.changes() };
        Ok(Outcome { rows: found, changes })
    }
}

impl Error {
    /// The constraint SQLite says this broke, if it broke one.
    fn constraint(&self) -> Option<Constraint> {
        let Self::Run { source: rusqlite::Error::SqliteFailure(failure, _), .. } = self else { return None };
        match failure.extended_code {
            ffi::SQLITE_CONSTRAINT_UNIQUE | ffi::SQLITE_CONSTRAINT_PRIMARYKEY => Some(Constraint::Unique),
            ffi::SQLITE_CONSTRAINT_FOREIGNKEY => Some(Constraint::Reference),
            ffi::SQLITE_CONSTRAINT_NOTNULL => Some(Constraint::Null),
            ffi::SQLITE_CONSTRAINT_CHECK => Some(Constraint::Check),
            _ => None,
        }
    }
}

impl From<Error> for db::Error<Error> {
    fn from(source: Error) -> Self {
        if let Error::Render { index, source } = source {
            return Self::Unrenderable { index, source };
        }
        match source.constraint() {
            Some(constraint) => Self::Constraint { constraint, source },
            None => Self::Backend { source },
        }
    }
}

impl Db for Sqlite {
    type Error = Error;

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for D1; this adapter does no waiting")]
    async fn run(&self, statement: Statement) -> Result<Outcome, db::Error<Error>> {
        let guard = self.lock()?;
        Ok(Execution { connection: &guard }.run(&statement, 0)?)
    }

    #[allow(clippy::unused_async_trait_impl, reason = "the port is async for D1; this adapter does no waiting")]
    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<Outcome>, db::Error<Error>> {
        let mut guard = self.lock()?;
        let transaction = guard.transaction().map_err(|source| Error::Run { stage: Stage::Begin, source })?;
        let execution = Execution { connection: &transaction };
        let outcomes = statements
            .iter()
            .enumerate()
            .map(|(index, statement)| execution.run(statement, index))
            .collect::<Result<Vec<_>, _>>()?;
        transaction.commit().map_err(|source| Error::Run { stage: Stage::Commit, source })?;
        trace::debug!(name: trace::event::DB_BATCH, statements = trace::Count::from(outcomes.len()).field());
        Ok(outcomes)
    }
}
