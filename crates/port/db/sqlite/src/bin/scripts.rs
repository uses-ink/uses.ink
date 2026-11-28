//! Writes the conformance suite's migrations as the files `wrangler d1 migrations apply` reads,
//! rendered by the port's one renderer, and what the SQLite adapter makes of the same migrations:
//! the schema it ends with and its ledger. `cargo xtask worker migrate` applies the files to a
//! local D1 and compares (decision 0098).
//!
//! `scripts <folder>` writes `migrations/NNNN_name.sql`, the two queries `schema.sql` and
//! `ledger.sql`, and `expected.tsv`, whose lines are `schema`, type, name, table and definition, or
//! `ledger` and a migration's name.

use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};

use db::{Db, Renderer, Statement, Unapplied, Unrenderable};
use db_conformance::Migrations;
use sea_query::{Expr, ExprTrait, IdenStatic, Order, Query};
use uses_ink_db_sqlite::Sqlite;

#[derive(Clone, Copy, IdenStatic)]
#[iden = "sqlite_master"]
enum Master {
    Table,
    Type,
    Name,
    #[iden = "tbl_name"]
    Owner,
    Sql,
}

/// What stopped the run.
#[derive(Debug)]
enum Fault {
    /// No folder was given.
    Usage,
    /// A file or folder could not be written.
    Write { path: PathBuf, source: std::io::Error },
    /// A statement could not be rendered.
    Render(Unrenderable),
    /// The SQLite adapter failed.
    Sqlite(uses_ink_db_sqlite::Error),
    /// The migrations did not apply.
    Apply(Unapplied<uses_ink_db_sqlite::Error>),
    /// A query failed.
    Query(db::Error<uses_ink_db_sqlite::Error>),
    /// The schema holds a column that is not text.
    Column(db::ColumnError),
    /// The text of the expected file could not be built.
    Text(fmt::Error),
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => out.write_str("usage: scripts <folder>"),
            Self::Write { path, .. } => write!(out, "cannot write {}", path.display()),
            Self::Render(_) => out.write_str("a statement cannot be written as text"),
            Self::Sqlite(_) => out.write_str("the SQLite adapter failed"),
            Self::Apply(_) => out.write_str("the migrations did not apply to SQLite"),
            Self::Query(_) => out.write_str("a query of the schema failed"),
            Self::Column(_) => out.write_str("the schema has a column that is not text"),
            Self::Text(_) => out.write_str("the expected file could not be built"),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Usage => None,
            Self::Write { source, .. } => Some(source),
            Self::Render(source) => Some(source),
            Self::Sqlite(source) => Some(source),
            Self::Apply(source) => Some(source),
            Self::Query(source) => Some(source),
            Self::Column(source) => Some(source),
            Self::Text(source) => Some(source),
        }
    }
}

/// The folder the files go to.
struct Folder(PathBuf);

impl Folder {
    fn put(&self, name: &str, text: &str) -> Result<(), Fault> {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Fault::Write { path: parent.to_path_buf(), source })?;
        }
        std::fs::write(&path, text).map_err(|source| Fault::Write { path, source })
    }

    fn query(&self, name: &str, statement: &Statement) -> Result<(), Fault> {
        let text = Renderer.inline(statement).map_err(Fault::Render)?;
        self.put(name, &format!("{text};\n"))
    }
}

impl From<db::ColumnError> for Fault {
    fn from(source: db::ColumnError) -> Self {
        Self::Column(source)
    }
}

impl From<fmt::Error> for Fault {
    fn from(source: fmt::Error) -> Self {
        Self::Text(source)
    }
}

/// The queries, which are the same for SQLite and for D1.
struct Queries;

impl Queries {
    /// What the migrations made, less the tables the platforms keep for themselves.
    fn schema() -> Statement {
        Query::select()
            .columns([Master::Type, Master::Name, Master::Owner, Master::Sql])
            .from(Master::Table)
            .and_where(Expr::col(Master::Name).not_like("sqlite_%"))
            .and_where(Expr::col(Master::Name).is_not_in(["d1_migrations", "_cf_KV"]))
            .order_by(Master::Name, Order::Asc)
            .into()
    }

    /// The names wrangler's ledger holds, as the port's ledger holds them.
    fn ledger() -> Statement {
        Query::select().column(db::Ledger::Name).from(db::Ledger::Table).order_by(db::Ledger::Id, Order::Asc).into()
    }
}

impl Folder {
    fn expected(&self) -> Result<(), Fault> {
        let steps = Migrations::steps();
        let sqlite = Sqlite::memory().map_err(Fault::Sqlite)?;
        db_conformance::pollster::block_on(steps.apply(&sqlite)).map_err(Fault::Apply)?;
        let mut tsv = String::new();
        let rows = db_conformance::pollster::block_on(sqlite.rows(Queries::schema())).map_err(Fault::Query)?;
        for row in rows {
            let [kind, name, owner, sql] =
                [Master::Type, Master::Name, Master::Owner, Master::Sql].map(|column| row.get::<String>(column));
            let (kind, name, owner, sql) = (kind?, name?, owner?, sql?);
            writeln!(tsv, "schema\t{kind}\t{name}\t{owner}\t{sql}")?;
        }
        let rows = db_conformance::pollster::block_on(sqlite.rows(Queries::ledger())).map_err(Fault::Query)?;
        for row in rows {
            writeln!(tsv, "ledger\t{}", row.get::<String>(db::Ledger::Name)?)?;
        }
        self.put("expected.tsv", &tsv)
    }
}

fn main() -> Result<(), Fault> {
    let folder = Folder(std::env::args_os().nth(1).map(|path| Path::new(&path).to_path_buf()).ok_or(Fault::Usage)?);
    for script in Migrations::steps().scripts().map_err(Fault::Render)? {
        folder.put(&format!("migrations/{}", script.name), &script.text)?;
    }
    folder.query("schema.sql", &Queries::schema())?;
    folder.query("ledger.sql", &Queries::ledger())?;
    folder.expected()
}
