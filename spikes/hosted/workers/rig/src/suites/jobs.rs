//! The queue port's suite, on the rows of D1 with a Queues binding for the wake-up.

use db::Steps;
use db_d1::D1;
use queue_cf::{Cf, Nudged, Queues, Refused};
use queue_conformance::Case;
use queue_rows::{Local, Queued};
use worker::{Env, Result};

/// The D1 and Queues bindings of the rig.
pub(crate) struct Jobs<'a> {
    pub(crate) env: &'a Env,
}

impl Jobs<'_> {
    /// The wake-up cases, which the native test of the adapter runs too.
    const NUDGED: &'static str = "nudges_after_what_makes_a_message_visible";
    const REFUSED: &'static str = "keeps_its_answers_when_the_wake_up_fails";

    /// The cases' names.
    pub(crate) fn names() -> Vec<&'static str> {
        let mut names: Vec<&'static str> = Case::ALL.iter().map(|case| case.name()).collect();
        names.extend([Self::NUDGED, Self::REFUSED]);
        names
    }

    /// A database with the queue's table and nothing in it.
    async fn migrated(&self) -> Result<D1> {
        let database = self.env.d1("DB")?;
        database.exec("DROP TABLE IF EXISTS queue_message\nDROP TABLE IF EXISTS d1_migrations").await?;
        let db = D1::new(database);
        let steps = Steps::new().with::<Queued>().map_err(|error| worker::Error::RustError(error.to_string()))?;
        steps.apply(&db).await.map_err(|error| worker::Error::RustError(error.to_string()))?;
        Ok(db)
    }

    /// Runs the case on a fresh queue; false if there is no such case.
    pub(crate) async fn run(&self, name: &str) -> Result<bool> {
        if name == Self::NUDGED {
            Nudged::check(self.migrated().await?).await;
        } else if name == Self::REFUSED {
            Refused::check(self.migrated().await?).await;
        } else if let Some(case) = Case::named(name) {
            let queue = Cf::new(Local::new(self.migrated().await?), Queues::new(self.env.queue("WAKE")?));
            case.run(&queue).await;
        } else {
            return Ok(false);
        }
        Ok(true)
    }
}
