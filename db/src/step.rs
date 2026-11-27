//! Migrations: typed values with a number and a name, applied in order through any [`Db`], with
//! wrangler's ledger table so a D1 database keeps one history.

use std::fmt;
use trace::Field as _;

use sea_query::{ColumnDef, Expr, IdenStatic, Order, Query, Table};

use crate::{ColumnError, Db, Error, Statement};

/// One migration: what it does to the schema, built from typed definitions.
pub trait Migration {
    /// Its place in the order. Numbers only rise.
    const NUMBER: u32;
    /// Its name: lower case letters, digits and `_`.
    const NAME: &'static str;

    /// The statements that apply it. They run with the ledger row as one atomic batch.
    fn statements() -> Vec<Statement>;
}

/// wrangler's ledger table, `d1_migrations`: one row per applied migration, by its ledger name.
#[derive(Clone, Copy, Debug, IdenStatic)]
#[iden = "d1_migrations"]
pub enum Ledger {
    /// The table.
    Table,
    /// The row number.
    Id,
    /// The ledger name.
    Name,
    /// When it was applied.
    AppliedAt,
}

#[derive(Clone, Copy, Debug)]
struct Step {
    number: u32,
    name: &'static str,
    statements: fn() -> Vec<Statement>,
}

impl Step {
    /// The name the ledger records, as wrangler records a file: `0001_words.sql`.
    fn ledger(&self) -> String {
        format!("{:04}_{}.sql", self.number, self.name)
    }
}

/// Why a migration cannot join a set.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StepError {
    /// A name that is not lower case words.
    Name {
        /// The name.
        name: &'static str,
    },
    /// A migration whose number does not come after the one before it.
    Order {
        /// The earlier migration's number.
        before: u32,
        /// The number that is not later.
        after: u32,
    },
}

impl fmt::Display for StepError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name { name } => write!(out, "`{name}` is not a migration name: lower case letters, digits and `_`"),
            Self::Order { before, after } => {
                write!(out, "migration {after} does not come after {before}: numbers only rise")
            }
        }
    }
}

impl std::error::Error for StepError {}

/// An ordered, checked set of migrations.
#[derive(Clone, Debug, Default)]
pub struct Steps {
    steps: Vec<Step>,
}

/// The migrations a run applied, by name, in order.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Applied {
    /// The names.
    pub names: Vec<String>,
}

/// A migration run that stopped.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unapplied<E> {
    /// The database failed. A step's statements and its ledger row are one batch, so the step
    /// that failed left nothing behind.
    Db(Error<E>),
    /// The ledger holds a migration this set does not know: the database is newer than the code.
    Unknown {
        /// The ledger's name for it.
        name: String,
    },
    /// The ledger holds a migration where this set has another: one was applied out of order,
    /// or the ledger skips one this set has before it.
    Order {
        /// The ledger's name at the first place it differs.
        held: String,
        /// The name this set has there.
        expected: String,
    },
    /// The ledger has a row that cannot be read.
    Ledger(ColumnError),
    /// The ledger's row for a migration could not be built.
    Statement(sea_query::error::Error),
}

impl<E> fmt::Display for Unapplied<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Db(_) => out.write_str("the migration ledger could not be updated"),
            Self::Unknown { name } => {
                write!(out, "the database has applied `{name}`, which this version does not know")
            }
            Self::Order { held, expected } => {
                write!(out, "the database has applied `{held}` where this version expects `{expected}`")
            }
            Self::Ledger(_) => out.write_str("the migration ledger has a row that cannot be read"),
            Self::Statement(_) => out.write_str("the migration ledger's row could not be built"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Unapplied<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Db(source) => Some(source),
            Self::Ledger(source) => Some(source),
            Self::Statement(source) => Some(source),
            Self::Unknown { .. } | Self::Order { .. } => None,
        }
    }
}

impl<E> From<Error<E>> for Unapplied<E> {
    fn from(error: Error<E>) -> Self {
        Self::Db(error)
    }
}

impl Steps {
    /// An empty set.
    pub const fn new() -> Self {
        Self { steps: Vec::new() }
    }

    /// The set with `M` after the ones it has. Its number must be higher than theirs.
    pub fn with<M: Migration>(mut self) -> Result<Self, StepError> {
        let named =
            !M::NAME.is_empty() && M::NAME.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        if !named {
            return Err(StepError::Name { name: M::NAME });
        }
        if let Some(last) = self.steps.last()
            && M::NUMBER <= last.number
        {
            return Err(StepError::Order { before: last.number, after: M::NUMBER });
        }
        self.steps.push(Step { number: M::NUMBER, name: M::NAME, statements: M::statements });
        Ok(self)
    }

    /// The ledger names of the migrations, in order.
    pub fn names(&self) -> impl Iterator<Item = String> + '_ {
        self.steps.iter().map(Step::ledger)
    }

    /// Whether the ledger's names are the first of the set's, in order.
    fn prefix<E>(held: &[String], known: &[String]) -> Result<(), Unapplied<E>> {
        match held.iter().zip(known).find(|(held, expected)| held != expected) {
            Some((held, expected)) => Err(Unapplied::Order { held: held.clone(), expected: expected.clone() }),
            None => Ok(()),
        }
    }

    /// Says a migration was applied.
    fn announce(name: &str, statements: usize) {
        trace::info!(
            name: trace::event::DB_MIGRATED,
            migration = trace::Shown(name).field(),
            statements = trace::Count::from(statements).field(),
        );
    }

    fn ledger_table() -> Statement {
        Statement::from(
            Table::create()
                .table(Ledger::Table)
                .if_not_exists()
                .col(ColumnDef::new(Ledger::Id).integer().not_null().auto_increment().primary_key())
                .col(ColumnDef::new(Ledger::Name).text().unique_key())
                .col(ColumnDef::new(Ledger::AppliedAt).timestamp().not_null().default(Expr::current_timestamp())),
        )
    }

    /// Applies every migration the ledger does not hold, in order, each with its ledger row as
    /// one atomic batch. A database that already has them all is left alone. A ledger that is
    /// not a prefix of this set (a name it does not know, or a gap before one it holds) is
    /// refused before anything runs.
    pub async fn apply<D: Db>(&self, db: &D) -> Result<Applied, Unapplied<D::Error>> {
        db.run(Self::ledger_table()).await?;
        let listing =
            Statement::from(Query::select().column(Ledger::Name).from(Ledger::Table).order_by(Ledger::Id, Order::Asc));
        let mut held = Vec::new();
        for row in db.rows(listing).await? {
            held.push(row.get::<String>(Ledger::Name).map_err(Unapplied::Ledger)?);
        }
        let known: Vec<String> = self.names().collect();
        if let Some(name) = held.iter().find(|name| !known.contains(name)) {
            return Err(Unapplied::Unknown { name: name.clone() });
        }
        Self::prefix(&held, &known)?;
        let mut applied = Applied::default();
        for (step, name) in self.steps.iter().zip(known) {
            if held.contains(&name) {
                continue;
            }
            let mut batch = (step.statements)();
            batch.push(Statement::from(
                Query::insert()
                    .into_table(Ledger::Table)
                    .columns([Ledger::Name])
                    .values([Expr::val(name.as_str())])
                    .map_err(Unapplied::Statement)?,
            ));
            let statements = batch.len();
            db.batch(batch).await?;
            Self::announce(&name, statements);
            applied.names.push(name);
        }
        Ok(applied)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    struct One;
    struct Two;
    struct Loud;

    impl Migration for One {
        const NUMBER: u32 = 1;
        const NAME: &'static str = "a";
        fn statements() -> Vec<Statement> {
            Vec::new()
        }
    }

    impl Migration for Two {
        const NUMBER: u32 = 1;
        const NAME: &'static str = "b";
        fn statements() -> Vec<Statement> {
            Vec::new()
        }
    }

    impl Migration for Loud {
        const NUMBER: u32 = 2;
        const NAME: &'static str = "A";
        fn statements() -> Vec<Statement> {
            Vec::new()
        }
    }

    #[test]
    fn checks_names_and_order() {
        assert_eq!(Steps::new().with::<One>().unwrap().names().collect::<Vec<_>>(), ["0001_a.sql"]);
        assert_eq!(
            Steps::new().with::<One>().unwrap().with::<Two>().unwrap_err(),
            StepError::Order { before: 1, after: 1 }
        );
        assert_eq!(Steps::new().with::<Loud>().unwrap_err(), StepError::Name { name: "A" });
    }
}
