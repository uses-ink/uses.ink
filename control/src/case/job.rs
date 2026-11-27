//! Build jobs: requesting a build, and working one off the queue.

use std::time::Duration;

use blob::Blob;
use build::{Build, BuildId, Outcome, Request};
use db::{Db, Statement};
use jiff::Timestamp;
use model::Hash;
use queue::{Delivery, Message, Queue, Sent};

use crate::deploy::{self, Published};
use crate::id::{Id, Site as SiteKind};
use crate::job::{self, Dropped, Job, Requested, State, Worked};
use crate::{Control, Error, Event, Failed, Ports, Refusal};

impl<P: Ports> Control<P> {
    /// How many times a build is tried before it is given up on.
    pub const ATTEMPTS: u32 = 3;
    /// How long a failed build waits before it is tried again.
    pub const RETRY: Duration = Duration::from_secs(30);
    /// How long after a deploy is published every reader sees it (architecture section 11).
    pub const VISIBLE: Duration = Duration::from_secs(10);

    /// Queues a build of a site. A request with the dedupe key of one still in the queue is
    /// dropped, so a forge that sends the same webhook twice builds once.
    pub async fn request_build(&self, new: job::New) -> Result<Requested, Error<P>> {
        const INSERT: &str = "INSERT INTO job (id, site_id, trigger, state, requested_at) VALUES (?, ?, ?, 'queued', ?)";
        let site = self.active(new.site).await?;
        let message = Message { body: new.id.to_string().into_bytes(), dedupe: new.dedupe };
        match self.ports.queue().send(message, new.at).await? {
            Sent::Duplicate(_) => Ok(Requested::Duplicate),
            _ => {
                let batch = vec![
                    Statement::prepare(INSERT).bind(new.id.to_string()).bind(site.id.to_string()).bind(job::Stored(new.trigger).name()).bind(new.at.as_millisecond()),
                    Event::BuildRequested { build: new.id }.insert(site.id, new.at).map_err(Error::Log)?,
                ];
                self.ports.db().batch(batch).await?;
                Ok(Requested::Queued(new.id))
            }
        }
    }

    /// The job for a build id, if there is one.
    pub async fn job(&self, id: BuildId) -> Result<Option<Job>, Error<P>> {
        const SELECT: &str = "SELECT id, site_id, trigger, state, requested_at, finished_at, deploy_id, log FROM job WHERE id = ?";
        match self.ports.db().first(Statement::prepare(SELECT).bind(id.to_string())).await? {
            Some(row) => Ok(Some(Job::try_from(&row)?)),
            None => Ok(None),
        }
    }

    /// The most recent `limit` jobs of a site, newest first.
    pub async fn jobs(&self, site: Id<SiteKind>, limit: u32) -> Result<Vec<Job>, Error<P>> {
        const SELECT: &str = "SELECT id, site_id, trigger, state, requested_at, finished_at, deploy_id, log FROM job \
            WHERE site_id = ? ORDER BY requested_at DESC, rowid DESC LIMIT ?";
        self.site(site).await?;
        let rows = self.ports.db().rows(Statement::prepare(SELECT).bind(site.to_string()).bind(limit)).await?;
        Ok(rows.iter().map(Job::try_from).collect::<Result<_, _>>()?)
    }

    /// Takes one request off the queue and runs it: builds the site's source, records the deploy
    /// and publishes it unless publishing is paused, stores the build's log as NDJSON in the
    /// blob store, and settles the request.
    ///
    /// A site that does not build is not a failure: the job ends `refused` and the log says why.
    /// A failure of the builder is retried up to [`Control::ATTEMPTS`] times, [`Control::RETRY`]
    /// apart, then the job ends `failed`; the error is returned either way. A request that names
    /// nothing, a build that has no job, or a site that is gone is dropped and said so.
    pub async fn work<W: Build>(&self, builder: &W, now: Timestamp, lease: Duration) -> Result<Worked, Failed<P, build::Error<W::Error>>> {
        let Some(delivery) = self.ports.queue().receive(now, lease).await.map_err(Error::from)? else {
            return Ok(Worked::Idle);
        };
        let Some(job) = self.claim(&delivery, now).await? else {
            return Ok(Worked::Dropped(self.drop_request(&delivery).await?));
        };
        let site = self.active(job.site).await?;
        let tenant = Self::tenant(site.id).map_err(Error::from)?;
        let base = self.origin.site(&site.handle).map_err(Error::Origin)?;
        let request = Request { build: job.id, tenant: tenant.clone(), trigger: job.trigger, source: site.source, base };
        let mut events: Vec<build::Event> = Vec::new();
        match builder.run(&request, &mut events).await {
            Err(error) => {
                self.give_up(&delivery, &job, now).await?;
                Err(Failed::Other(error))
            }
            Ok(Outcome::Built(built)) => {
                let recorded = self.deploy(deploy::New { site: site.id, manifest: built.manifest, build: Some(job.id), at: now }).await?;
                if let Published::Live { previous } = recorded.published {
                    events.push(build::Event::Published { deploy: recorded.deploy, previous, visible_within: Self::VISIBLE });
                }
                let log = self.log(&tenant, &events).await?;
                self.finish(&job, State::Succeeded, Some(recorded.deploy), log, now).await?;
                self.ports.queue().ack(&delivery).await.map_err(Error::from)?;
                Ok(Worked::Built { build: job.id, recorded })
            }
            Ok(_) => {
                let log = self.log(&tenant, &events).await?;
                self.finish(&job, State::Refused, None, log, now).await?;
                self.ports.queue().ack(&delivery).await.map_err(Error::from)?;
                Ok(Worked::Refused { build: job.id })
            }
        }
    }

    /// The job a delivery is for, marked running; none if the request is not one this plane can
    /// work, in which case [`Control::drop_request`] says why.
    async fn claim(&self, delivery: &Delivery, _now: Timestamp) -> Result<Option<Job>, Error<P>> {
        const RUNNING: &str = "UPDATE job SET state = 'running' WHERE id = ? AND state IN ('queued', 'running')";
        let Ok(id) = String::from_utf8_lossy(&delivery.body).parse::<BuildId>() else { return Ok(None) };
        let Some(job) = self.job(id).await? else { return Ok(None) };
        match self.site(job.site).await {
            Ok(site) if site.state == crate::site::State::Active => {}
            Ok(_) | Err(Error::Refused(Refusal::NoSite { .. })) => return Ok(None),
            Err(error) => return Err(error),
        }
        self.ports.db().run(Statement::prepare(RUNNING).bind(id.to_string())).await?;
        Ok(Some(Job { state: State::Running, ..job }))
    }

    /// Settles a request that cannot be worked, and says why it could not.
    async fn drop_request(&self, delivery: &Delivery) -> Result<Dropped, Error<P>> {
        let dropped = match String::from_utf8(delivery.body.clone()).ok().and_then(|text| text.parse::<BuildId>().ok()) {
            None => Dropped::Unnamed,
            Some(build) => match self.job(build).await? {
                None => Dropped::NoJob { build },
                Some(job) => Dropped::NoSite { build, site: job.site },
            },
        };
        self.ports.queue().ack(delivery).await?;
        Ok(dropped)
    }

    /// After a builder failure: gives the request back for another try, or ends the job.
    async fn give_up(&self, delivery: &Delivery, job: &Job, now: Timestamp) -> Result<(), Error<P>> {
        if delivery.attempt < Self::ATTEMPTS {
            self.ports.queue().release(delivery, now, Self::RETRY).await?;
            return Ok(());
        }
        self.finish(job, State::Failed, None, None, now).await?;
        self.ports.queue().ack(delivery).await?;
        Ok(())
    }

    /// Writes a build's events, one JSON object a line, as a blob of the site's tenant.
    async fn log(&self, tenant: &blob::Tenant, events: &[build::Event]) -> Result<Option<Hash>, Error<P>> {
        let mut lines = Vec::new();
        for event in events {
            serde_json::to_writer(&mut lines, event).map_err(Error::Log)?;
            lines.push(b'\n');
        }
        let hash = Hash::of(&lines);
        self.ports.blob().put(tenant, hash, [lines]).await?;
        Ok(Some(hash))
    }

    /// Ends a job.
    async fn finish(&self, job: &Job, state: State, deploy: Option<model::DeployId>, log: Option<Hash>, now: Timestamp) -> Result<(), Error<P>> {
        const FINISH: &str = "UPDATE job SET state = ?, finished_at = ?, deploy_id = ?, log = ? WHERE id = ?";
        let finish = Statement::prepare(FINISH)
            .bind(state.name())
            .bind(now.as_millisecond())
            .bind(deploy.map(|deploy| deploy.to_string()))
            .bind(log.map(|hash| hash.to_string()))
            .bind(job.id.to_string());
        self.ports.db().run(finish).await?;
        Ok(())
    }
}
