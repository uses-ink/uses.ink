//! Migrations: wrangler's folder of numbered `.sql` files, applied through any [`Db`].

use std::fmt;

use crate::script::ScriptError;
use crate::{ColumnError, Db, Error, Script, Statement};

/// One migration file: its name in wrangler's form, `0001_name.sql`, and its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    name: &'static str,
    script: &'static str,
}

impl Step {
    /// A step; `script` is usually `include_str!` of the file `name` names. It is checked by
    /// [`Steps::new`].
    pub const fn new(name: &'static str, script: &'static str) -> Self {
        Self { name, script }
    }

    /// The file's name, which is what the ledger records.
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The number a name starts with, if the name is `NNNN_words.sql` in lower case.
    fn number(&self) -> Option<u32> {
        let (digits, rest) = self.name.split_once('_')?;
        let words = rest.strip_suffix(".sql")?;
        let named = !words.is_empty() && words.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
        (digits.len() == 4 && digits.bytes().all(|b| b.is_ascii_digit()) && named).then(|| digits.parse().ok())?
    }
}

/// Why a list of steps is not a migration set.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum StepError {
    /// A name that is not `NNNN_words.sql`.
    Name {
        /// The name.
        name: &'static str,
    },
    /// A step that does not come after the one before it.
    Order {
        /// The earlier step's name.
        before: &'static str,
        /// The step that is not later.
        after: &'static str,
    },
    /// A file that is not a script D1 would run.
    Script {
        /// The step.
        name: &'static str,
        /// What the script splitter refused.
        source: ScriptError,
    },
}

impl fmt::Display for StepError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name { name } => write!(out, "`{name}` is not a migration name: it is `NNNN_words.sql` in lower case"),
            Self::Order { before, after } => write!(out, "`{after}` does not come after `{before}`: numbers only rise"),
            Self::Script { name, .. } => write!(out, "`{name}` is not a script"),
        }
    }
}

impl std::error::Error for StepError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Script { source, .. } => Some(source),
            Self::Name { .. } | Self::Order { .. } => None,
        }
    }
}

/// An ordered, checked set of migrations.
#[derive(Clone, Debug, PartialEq)]
pub struct Steps {
    steps: Vec<(Step, Script)>,
}

/// The migrations a run applied, by name, in order.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Applied {
    /// The names.
    pub names: Vec<&'static str>,
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
    /// The ledger has a row that cannot be read.
    Ledger(ColumnError),
}

impl<E> fmt::Display for Unapplied<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Db(_) => out.write_str("the migration ledger could not be updated"),
            Self::Unknown { name } => write!(out, "the database has applied `{name}`, which this version does not know"),
            Self::Ledger(_) => out.write_str("the migration ledger has a row that cannot be read"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Unapplied<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Db(source) => Some(source),
            Self::Ledger(source) => Some(source),
            Self::Unknown { .. } => None,
        }
    }
}

impl<E> From<Error<E>> for Unapplied<E> {
    fn from(error: Error<E>) -> Self {
        Self::Db(error)
    }
}

impl Steps {
    /// wrangler's ledger, as `wrangler d1 migrations apply` creates it.
    const CREATE: &'static str = "CREATE TABLE IF NOT EXISTS d1_migrations(\
        id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT UNIQUE, applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL)";
    const LIST: &'static str = "SELECT name FROM d1_migrations ORDER BY id";
    const RECORD: &'static str = "INSERT INTO d1_migrations (name) VALUES (?)";

    /// Checks `steps`: named as wrangler names them, numbers rising, each a script D1 runs.
    pub fn new(steps: impl IntoIterator<Item = Step>) -> Result<Self, StepError> {
        let mut checked: Vec<(Step, Script)> = Vec::new();
        let mut last = None;
        for step in steps {
            let number = step.number().ok_or(StepError::Name { name: step.name })?;
            if let Some((before, last_number)) = last
                && number <= last_number
            {
                return Err(StepError::Order { before, after: step.name });
            }
            last = Some((step.name, number));
            let script = Script::parse(step.script).map_err(|source| StepError::Script { name: step.name, source })?;
            checked.push((step, script));
        }
        Ok(Self { steps: checked })
    }

    /// The steps, in order.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.steps.iter().map(|(step, _)| step.name)
    }

    /// Applies every step the ledger does not hold, in order, each with its ledger row as one
    /// atomic batch. A database that already has them all is left alone.
    pub async fn apply<D: Db>(&self, db: &D) -> Result<Applied, Unapplied<D::Error>> {
        db.run(Statement::prepare(Self::CREATE)).await?;
        let mut held = Vec::new();
        for row in db.rows(Statement::prepare(Self::LIST)).await? {
            held.push(row.get::<String>("name").map_err(Unapplied::Ledger)?);
        }
        if let Some(name) = held.iter().find(|name| !self.steps.iter().any(|(step, _)| step.name == name.as_str())) {
            return Err(Unapplied::Unknown { name: name.clone() });
        }
        let mut applied = Applied::default();
        for (step, script) in &self.steps {
            if held.iter().any(|name| name == step.name) {
                continue;
            }
            let mut batch = script.statements().to_vec();
            batch.push(Statement::prepare(Self::RECORD).bind(step.name));
            db.batch(batch).await?;
            applied.names.push(step.name);
        }
        Ok(applied)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn checks_names_and_order() {
        let ok = |name| Step::new(name, "SELECT 1");
        assert!(Steps::new([ok("0001_a.sql"), ok("0002_b.sql")]).is_ok());
        assert_eq!(Steps::new([ok("1_a.sql")]).unwrap_err(), StepError::Name { name: "1_a.sql" });
        assert_eq!(Steps::new([ok("0001_A.sql")]).unwrap_err(), StepError::Name { name: "0001_A.sql" });
        assert_eq!(Steps::new([ok("0001_a")]).unwrap_err(), StepError::Name { name: "0001_a" });
        assert_eq!(
            Steps::new([ok("0002_b.sql"), ok("0002_c.sql")]).unwrap_err(),
            StepError::Order { before: "0002_b.sql", after: "0002_c.sql" }
        );
    }

    #[test]
    fn refuses_a_script_d1_would_refuse() {
        let error = Steps::new([Step::new("0001_a.sql", "BEGIN;")]).unwrap_err();
        assert!(matches!(error, StepError::Script { name: "0001_a.sql", .. }));
    }
}
