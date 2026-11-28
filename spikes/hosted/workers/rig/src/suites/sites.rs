//! The edge's suite for `Sites` and `Blobs`, on D1 rows and an R2 bucket through the Worker's adapters.

use std::sync::Arc;

use blob_r2::{Limits, R2};
use control::{Domain, Origin, Remembered, Treed};
use db::Steps;
use db_d1::D1;
use edge_conformance::{Case, Outcome};
use edge_worker::{Hosted, Platform};
use queue_cf::{Cf, Queues};
use queue_rows::{Local, Queued};
use secret_worker::Worker;
use worker::{D1Database, Env, Result};

use crate::suites::Blobs;

/// The D1, R2 and Queues bindings of the rig, the stores of one control plane.
pub(crate) struct Sites<'a> {
    pub(crate) env: &'a Env,
}

impl Sites<'_> {
    /// The cases' names.
    pub(crate) fn names() -> Vec<&'static str> {
        Case::ALL.iter().map(|case| case.name()).collect()
    }

    /// The error of a set-up, as the rig's.
    fn rig(error: impl std::error::Error) -> worker::Error {
        worker::Error::RustError(format!("{error}: {:?}", error.source().map(ToString::to_string)))
    }

    /// Drops every table: a table that others point at waits for a round in which they are gone.
    async fn clear(database: &D1Database) -> Result<()> {
        for _ in 0..16 {
            let listed = database
                .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_cf_%'")
                .all()
                .await?;
            let names: Vec<serde_json::Value> = listed.results()?;
            if names.is_empty() {
                return Ok(());
            }
            for row in names {
                if let Some(name) = row.get("name").and_then(serde_json::Value::as_str) {
                    // a failure here is a table another still points at: the next round drops it
                    let _ = database.exec(&format!("DROP TABLE IF EXISTS \"{name}\"")).await;
                }
            }
        }
        Err(worker::Error::RustError("the tables of the rig could not all be dropped".to_owned()))
    }

    /// Runs the case on empty stores; none if there is no such case.
    pub(crate) async fn run(&self, name: &str) -> Result<Option<Outcome>> {
        let Some(case) = Case::named(name) else { return Ok(None) };
        let database = self.env.d1("DB")?;
        Self::clear(&database).await?;
        let db = Arc::new(D1::new(database));
        let steps = Steps::new()
            .with::<Domain>()
            .and_then(Steps::with::<Remembered>)
            .and_then(Steps::with::<Treed>)
            .and_then(Steps::with::<Queued>)
            .and_then(control::Schema::after)
            .map_err(Self::rig)?;
        steps.apply(&db).await.map_err(Self::rig)?;
        Blobs::clear(&self.env.bucket("BUCKET")?).await?;
        let limits = || Limits::new(Limits::PART, 3).map_err(|refused| worker::Error::RustError(refused.to_string()));
        let queue = Cf::new(Local::new(Arc::clone(&db)), Queues::new(self.env.queue("WAKE")?));
        let secret = Worker::from_env(self.env, Worker::KEY).map_err(Self::rig)?;
        let ports = Arc::new((Arc::clone(&db), R2::with(self.env.bucket("BUCKET")?, limits()?), queue, secret));
        let origin = model::ExternalUrl::parse("https://example.test").map_err(Self::rig)?;
        let origin = Origin::try_from(&origin).map_err(Self::rig)?;
        let blobs = R2::with(self.env.bucket("BUCKET")?, limits()?);
        let hosted = Hosted::new(ports, blobs, origin, Platform);
        Ok(Some(case.run(&hosted).await))
    }
}
