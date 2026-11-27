//! The adapter.

use std::fmt;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use db::{Constraint, Db, Outcome, Row, Statement, Value};
use rusqlite::types::{Value as Stored, ValueRef};
use rusqlite::{Connection, ffi};

use crate::Error;

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
    fn stored(value: &Value) -> Stored {
        match value {
            Value::Null => Stored::Null,
            Value::Integer(n) => Stored::Integer(*n),
            Value::Real(n) => Stored::Real(*n),
            Value::Text(text) => Stored::Text(text.clone()),
            Value::Blob(bytes) => Stored::Blob(bytes.clone()),
            // `Value` is non-exhaustive; every kind D1 knows is above
            _ => Stored::Null,
        }
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

    fn run(&self, statement: &Statement) -> Result<Outcome, Error> {
        let sql = statement.sql();
        let failed = |source| Error::Run { sql, source };
        let mut prepared = self.connection.prepare_cached(sql).map_err(failed)?;
        let readonly = prepared.readonly();
        let names: Vec<String> = prepared.column_names().into_iter().map(str::to_owned).collect();
        let bound = rusqlite::params_from_iter(statement.parameters().iter().map(Self::stored));
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
        match source.constraint() {
            Some(constraint) => Self::Constraint { constraint, source },
            None => Self::Backend { source },
        }
    }
}

impl Db for Sqlite {
    type Error = Error;

    async fn run(&self, statement: Statement) -> Result<Outcome, db::Error<Error>> {
        statement.check().map_err(|found| db::Error::Parameters { index: 0, found })?;
        let guard = self.lock()?;
        Ok(Execution { connection: &guard }.run(&statement)?)
    }

    async fn batch(&self, statements: Vec<Statement>) -> Result<Vec<Outcome>, db::Error<Error>> {
        for (index, statement) in statements.iter().enumerate() {
            statement.check().map_err(|found| db::Error::Parameters { index, found })?;
        }
        let mut guard = self.lock()?;
        let open = |source| Error::Run { sql: "BEGIN", source };
        let transaction = guard.transaction().map_err(open)?;
        let execution = Execution { connection: &transaction };
        let outcomes = statements.iter().map(|statement| execution.run(statement)).collect::<Result<Vec<_>, _>>()?;
        transaction.commit().map_err(|source| Error::Run { sql: "COMMIT", source })?;
        Ok(outcomes)
    }
}
