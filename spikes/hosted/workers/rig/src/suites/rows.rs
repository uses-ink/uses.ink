//! The row port's suite, on a D1 binding.

use db_conformance::Case;
use db_d1::D1;
use worker::{Env, Result};

/// The D1 binding of the rig, and the cases to run on it.
pub(crate) struct Rows<'a> {
    pub(crate) env: &'a Env,
}

impl Rows<'_> {
    /// The tables the cases make, children first: what a case leaves behind is dropped before the next.
    const TABLES: [&'static str; 8] = ["child", "parent", "every", "big", "alpha", "beta", "gamma", "d1_migrations"];

    /// The cases' names.
    pub(crate) fn names() -> Vec<&'static str> {
        Case::ALL.iter().map(|case| case.name()).collect()
    }

    /// Empties the database and runs the case; false if there is no such case.
    pub(crate) async fn run(&self, name: &str) -> Result<bool> {
        let Some(case) = Case::named(name) else { return Ok(false) };
        let database = self.env.d1("DB")?;
        let drops: Vec<String> = Self::TABLES.iter().map(|table| format!("DROP TABLE IF EXISTS {table}")).collect();
        database.exec(&drops.join("\n")).await?;
        case.run(&D1::new(database)).await;
        Ok(true)
    }
}
