//! Build jobs: requesting a build, and working one off the queue.

use std::time::Duration;

use blob::Blob;
use build::{Build, BuildId, Outcome, Request};
use db::Db;
use jiff::Timestamp;
use model::Hash;
use queue::{Delivery, Message, Queue, Sent};
use sea_query::{Expr, ExprTrait, Order, Query};

use crate::deploy::{self, Published};
use crate::id::{Id, Site as SiteKind};
use crate::job::{self, Dropped, Job, Requested, State, Worked};
use crate::{Control, Error, Event, Failed, Ports, Refusal, table};

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
        let site = self.active(new.site).await?;
        let message = Message { body: new.id.to_string().into_bytes(), dedupe: new.dedupe };
        if let Sent::Duplicate(_) = self.ports.queue().send(message, new.at).await? {
            return Ok(Requested::Duplicate);
        }
        let insert = Query::insert()
            .into_table(table::Job::Table)
            .columns([
                table::Job::Id,
                table::Job::SiteId,
                table::Job::Trigger,
                table::Job::State,
                table::Job::RequestedAt,
            ])
            .values([
                Expr::val(new.id.to_string()),
                Expr::val(site.id.to_string()),
                Expr::val(job::Stored(new.trigger).name()),
                Expr::val(State::Queued.name()),
                Expr::val(new.at.as_millisecond()),
            ])?
            .to_owned();
        let batch = vec![insert.into(), Event::BuildRequested { build: new.id }.insert(site.id, new.at)?];
        self.ports.db().batch(batch).await?;
        Ok(Requested::Queued(new.id))
    }

    /// A select of every column of a job, for the jobs whose `column` is `value`.
    fn jobs_where(column: table::Job, value: String) -> sea_query::SelectStatement {
        Query::select()
            .columns([
                table::Job::Id,
                table::Job::SiteId,
                table::Job::Trigger,
                table::Job::State,
                table::Job::RequestedAt,
                table::Job::FinishedAt,
                table::Job::DeployId,
                table::Job::Log,
            ])
            .from(table::Job::Table)
            .and_where(Expr::col(column).eq(value))
            .to_owned()
    }

    /// The job for a build id, if there is one.
    pub async fn job(&self, id: BuildId) -> Result<Option<Job>, Error<P>> {
        match self.ports.db().first(Self::jobs_where(table::Job::Id, id.to_string()).into()).await? {
            Some(row) => Ok(Some(Job::try_from(&row)?)),
            None => Ok(None),
        }
    }

    /// The most recent `limit` jobs of a site, newest first.
    pub async fn jobs(&self, site: Id<SiteKind>, limit: u32) -> Result<Vec<Job>, Error<P>> {
        let select = Self::jobs_where(table::Job::SiteId, site.to_string())
            .order_by(table::Job::RequestedAt, Order::Desc)
            .order_by(table::Row::Rowid, Order::Desc)
            .limit(u64::from(limit))
            .to_owned();
        self.site(site).await?;
        let rows = self.ports.db().rows(select.into()).await?;
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
    pub async fn work<W: Build>(
        &self,
        builder: &W,
        now: Timestamp,
        lease: Duration,
    ) -> Result<Worked, Failed<P, build::Error<W::Error>>> {
        let Some(delivery) = self.ports.queue().receive(now, lease).await.map_err(Error::from)? else {
            return Ok(Worked::Idle);
        };
        let Some(job) = self.claim(&delivery, now).await? else {
            return Ok(Worked::Dropped(self.drop_request(&delivery).await?));
        };
        let site = self.active(job.site).await?;
        let tenant = Self::tenant(site.id).map_err(Error::from)?;
        let base = self.origin.site(&site.handle).map_err(Error::Origin)?;
        let request =
            Request { build: job.id, tenant: tenant.clone(), trigger: job.trigger, source: site.source, base };
        let mut events: Vec<build::Event> = Vec::new();
        match builder.run(&request, &mut events).await {
            Err(error) => {
                self.give_up(&delivery, &job, now).await?;
                Err(Failed::Other(error))
            }
            Ok(outcome) => Ok(self.settle(Settling { job, delivery, tenant, events, now }, outcome).await?),
        }
    }

    /// Ends a job whose build ran: records and publishes what it made, stores its log, and acks
    /// the request.
    async fn settle(&self, running: Settling, outcome: Outcome) -> Result<Worked, Error<P>> {
        let Settling { job, delivery, tenant, mut events, now } = running;
        let (ended, worked) = match outcome {
            Outcome::Built(built) => {
                let new = deploy::New { site: job.site, manifest: built.manifest, build: Some(job.id), at: now };
                let recorded = self.deploy(new).await?;
                if let Published::Live { previous } = recorded.published {
                    let visible_within = Self::VISIBLE;
                    events.push(build::Event::Published { deploy: recorded.deploy, previous, visible_within });
                }
                (State::Succeeded, Worked::Built { build: job.id, recorded })
            }
            _ => (State::Refused, Worked::Refused { build: job.id }),
        };
        let deploy = match &worked {
            Worked::Built { recorded, .. } => Some(recorded.deploy),
            _ => None,
        };
        let log = self.log(&tenant, &events).await?;
        self.finish(&job, Ended { state: ended, deploy, log }, now).await?;
        self.ports.queue().ack(&delivery).await?;
        Ok(worked)
    }

    /// The job a delivery is for, marked running; none if the request is not one this plane can
    /// work, in which case [`Control::drop_request`] says why.
    async fn claim(&self, delivery: &Delivery, _now: Timestamp) -> Result<Option<Job>, Error<P>> {
        let Ok(id) = String::from_utf8_lossy(&delivery.body).parse::<BuildId>() else { return Ok(None) };
        let Some(job) = self.job(id).await? else { return Ok(None) };
        match self.site(job.site).await {
            Ok(site) if site.state == crate::site::State::Active => {}
            Ok(_) | Err(Error::Refused(Refusal::NoSite { .. })) => return Ok(None),
            Err(error) => return Err(error),
        }
        let running = Query::update()
            .table(table::Job::Table)
            .value(table::Job::State, State::Running.name())
            .and_where(Expr::col(table::Job::Id).eq(id.to_string()))
            .and_where(Expr::col(table::Job::State).is_in([State::Queued.name(), State::Running.name()]))
            .to_owned();
        self.ports.db().run(running.into()).await?;
        Ok(Some(Job { state: State::Running, ..job }))
    }

    /// Settles a request that cannot be worked, and says why it could not.
    async fn drop_request(&self, delivery: &Delivery) -> Result<Dropped, Error<P>> {
        let dropped = match String::from_utf8(delivery.body.clone()).ok().and_then(|text| text.parse::<BuildId>().ok())
        {
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
        self.finish(job, Ended { state: State::Failed, deploy: None, log: None }, now).await?;
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
    async fn finish(&self, job: &Job, ended: Ended, now: Timestamp) -> Result<(), Error<P>> {
        let finish = Query::update()
            .table(table::Job::Table)
            .value(table::Job::State, ended.state.name())
            .value(table::Job::FinishedAt, now.as_millisecond())
            .value(table::Job::DeployId, ended.deploy.map(|deploy| deploy.to_string()))
            .value(table::Job::Log, ended.log.map(|hash| hash.to_string()))
            .and_where(Expr::col(table::Job::Id).eq(job.id.to_string()))
            .to_owned();
        self.ports.db().run(finish.into()).await?;
        Ok(())
    }
}

/// A job whose build ran and has not been settled yet.
#[derive(Debug)]
struct Settling {
    job: Job,
    delivery: Delivery,
    tenant: blob::Tenant,
    events: Vec<build::Event>,
    now: Timestamp,
}

/// How a job ended.
#[derive(Clone, Copy, Debug)]
struct Ended {
    state: State,
    deploy: Option<model::DeployId>,
    log: Option<Hash>,
}
