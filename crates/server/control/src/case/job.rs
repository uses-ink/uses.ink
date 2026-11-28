//! Build jobs: requesting a build, and working one off the queue.

use std::time::Duration;

use blob::Blob;
use build::{Build, BuildId, Outcome, Request};
use db::Db;
use jiff::Timestamp;
use logging::{Field as _, Instrument as _};
use model::Hash;
use queue::{Delivery, Message, Queue, Sent};
use sea_query::{Expr, ExprTrait, Order, Query};

use crate::deploy::{self, Published};
use crate::id::{Id, Site as SiteKind};
use crate::job::{self, Dropped, Job, Requested, State, Worked};
use crate::{Control, Error, Event, Failed, NewPreview, Ports, Refusal, Slug, Unreadable, table};

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
        self.queued(new, Attached::Nothing).await
    }

    /// Queues the build of a branch at one commit, for a preview (decision 0091): the job reads
    /// the branch `pin.branch` of the site's git source and ends as a preview labelled by the branch, never
    /// as a deploy. The same branch at the same commit is queued once however often it is asked
    /// (the dedupe key says so while it is queued, and [`Control::pinned`] after).
    pub async fn request_branch(&self, new: job::New, pin: build::Pin) -> Result<Requested, Error<P>> {
        Slug::branch(pin.branch.as_str()).map_err(Error::Label)?;
        let site = self.active(new.site).await?;
        self.previewable(&site).await?;
        self.queued(new, Attached::Pin(Box::new(pin))).await
    }

    /// Whether a build of `pin` was ever requested for `site`: the watch asks before it queues,
    /// so a restart does not build every branch again.
    pub async fn pinned(&self, site: Id<SiteKind>, pin: &build::Pin) -> Result<bool, Error<P>> {
        let select = Query::select()
            .column(table::Pin::JobId)
            .from(table::Pin::Table)
            .and_where(Expr::col(table::Pin::SiteId).eq(site.to_string()))
            .and_where(Expr::col(table::Pin::Branch).eq(pin.branch.as_str()))
            .and_where(Expr::col(table::Pin::Revision).eq(pin.revision.as_str()))
            .limit(1)
            .to_owned();
        Ok(self.ports.db().first(select.into()).await?.is_some())
    }

    /// Queues a build of the tree an upload sent (`ink push`): the tree is a blob of the site's
    /// tenant, and so is every file it lists, which the caller has checked are held. The build
    /// reads this tree, not whatever a later push sends.
    pub async fn request_upload(&self, new: job::New, tree: Hash) -> Result<Requested, Error<P>> {
        self.queued(new, Attached::Tree(tree)).await
    }

    /// The tree of the most recent push to `site`, if there was one: the one `ink export
    /// --source` returns.
    pub async fn latest_tree(&self, site: Id<SiteKind>) -> Result<Option<Hash>, Error<P>> {
        let select = Query::select()
            .column(table::Bundle::Tree)
            .from(table::Bundle::Table)
            .and_where(Expr::col(table::Bundle::SiteId).eq(site.to_string()))
            .order_by(table::Row::Rowid, Order::Desc)
            .limit(1)
            .to_owned();
        self.site(site).await?;
        self.read_tree(select).await
    }

    /// The tree the build `id` reads, when it is a build of a push.
    async fn tree_of(&self, id: BuildId) -> Result<Option<Hash>, Error<P>> {
        let select = Query::select()
            .column(table::Bundle::Tree)
            .from(table::Bundle::Table)
            .and_where(Expr::col(table::Bundle::JobId).eq(id.to_string()))
            .to_owned();
        self.read_tree(select).await
    }

    async fn read_tree(&self, select: sea_query::SelectStatement) -> Result<Option<Hash>, Error<P>> {
        match self.ports.db().first(select.into()).await? {
            Some(row) => Ok(crate::fields::Fields(&row).maybe_hash(table::Bundle::Tree)?),
            None => Ok(None),
        }
    }

    async fn queued(&self, new: job::New, attached: Attached) -> Result<Requested, Error<P>> {
        let site = self.active(new.site).await?;
        self.build_room(site.account, new.at).await?;
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
        let mut batch = vec![insert.into(), Event::BuildRequested { build: new.id }.insert(site.id, new.at)?];
        batch.extend(attached.insert(new.id, site.id)?);
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
            Some(row) => {
                let mut jobs = [Job::try_from(&row)?];
                self.pins(&mut jobs).await?;
                let [job] = jobs;
                Ok(Some(job))
            }
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
        let mut jobs = rows.iter().map(Job::try_from).collect::<Result<Vec<_>, _>>()?;
        self.pins(&mut jobs).await?;
        Ok(jobs)
    }

    /// Fills the pin of the jobs that build a branch, in one read of the site's pins.
    async fn pins(&self, jobs: &mut [Job]) -> Result<(), Error<P>> {
        let Some(first) = jobs.first() else { return Ok(()) };
        let site = first.site;
        let ids: Vec<String> = jobs.iter().map(|job| job.id.to_string()).collect();
        let select = Query::select()
            .columns([table::Pin::JobId, table::Pin::Branch, table::Pin::Revision])
            .from(table::Pin::Table)
            .and_where(Expr::col(table::Pin::SiteId).eq(site.to_string()))
            .and_where(Expr::col(table::Pin::JobId).is_in(ids))
            .to_owned();
        for row in self.ports.db().rows(select.into()).await? {
            let fields = crate::fields::Fields(&row);
            let id =
                fields.text(table::Pin::JobId)?.parse::<BuildId>().map_err(|source| Unreadable::Build { source })?;
            let branch = fields
                .text(table::Pin::Branch)?
                .parse()
                .map_err(|source| Unreadable::Revision { column: "branch", source })?;
            let revision = fields
                .text(table::Pin::Revision)?
                .parse()
                .map_err(|source| Unreadable::Revision { column: "revision", source })?;
            if let Some(job) = jobs.iter_mut().find(|job| job.id == id) {
                job.pin = Some(build::Pin { branch, revision });
            }
        }
        Ok(())
    }

    /// Takes one request off the queue and runs it: builds the site's source, records the deploy
    /// and publishes it unless publishing is paused, stores the build's log as NDJSON in the
    /// blob store, and settles the request.
    ///
    /// A site that does not build is not a failure: the job ends `refused` and the log says why.
    /// A failure of the builder is retried up to [`Control::ATTEMPTS`] times, [`Control::RETRY`]
    /// apart, then the job ends `failed`; the error is returned either way. So is a failure after
    /// the build (recording the deploy, storing the log): it counts as an attempt too, so a
    /// request that cannot be settled is not redelivered forever. A request that names nothing,
    /// a build that has no job, a job that already ended or a site that is gone is dropped and
    /// said so.
    pub async fn work<W: Build>(
        &self,
        builder: &W,
        now: Timestamp,
        lease: Duration,
    ) -> Result<Worked, Failed<P, build::Error<W::Error>>>
    where
        P: 'static,
    {
        let claimed = match self.take(now, lease).await? {
            Taken::Idle => return Ok(Worked::Idle),
            Taken::Dropped(dropped) => return Ok(Worked::Dropped(dropped)),
            Taken::Job(claimed) => *claimed,
        };
        let span = logging::job!(claimed.job.id, claimed.job.site, claimed.delivery.attempt);
        self.attempt(builder, claimed).instrument(span).await
    }

    /// The next request off the queue, claimed when it names a job that can run.
    async fn take(&self, now: Timestamp, lease: Duration) -> Result<Taken, Error<P>> {
        let Some(delivery) = self.ports.queue().receive(now, lease).await? else { return Ok(Taken::Idle) };
        match self.claim(&delivery, now).await? {
            Some(job) => Ok(Taken::Job(Box::new(Claimed { delivery, job, now }))),
            None => Ok(Taken::Dropped(self.drop_request(&delivery).await?)),
        }
    }

    /// Says an attempt failed, with its whole chain, and whether the queue will try again.
    fn lost(delivery: &Delivery, error: &(dyn std::error::Error + 'static)) {
        logging::error!(
            name: logging::event::JOB_FAILED,
            retry = delivery.attempt < Self::ATTEMPTS,
            error = logging::Failure(error).field(),
        );
    }

    /// One claimed job, from its builder to its settlement; the caller holds its `job` span.
    async fn attempt<W: Build>(
        &self,
        builder: &W,
        claimed: Claimed,
    ) -> Result<Worked, Failed<P, build::Error<W::Error>>>
    where
        P: 'static,
    {
        let Claimed { delivery, job, now } = claimed;
        logging::info!(
            name: logging::event::JOB_CLAIMED,
            trigger = logging::Kind::kind(job.trigger),
            tries = logging::Count(u64::from(Self::ATTEMPTS)).field(),
        );
        let site = self.active(job.site).await?;
        let tenant = Self::tenant(site.id).map_err(Error::from)?;
        let base = self.origin.site(&site.handle).map_err(Error::Origin)?;
        let source = match self.reads(site.source, &job).await {
            Ok(source) => source,
            Err(error) => {
                Self::lost(&delivery, &error);
                self.give_up(&delivery, &job, now).await?;
                return Err(Failed::Control(error));
            }
        };
        let pin = job.pin.clone();
        let request = Request { build: job.id, tenant: tenant.clone(), trigger: job.trigger, source, pin, base };
        let settling = Settling { job, tenant, events: Vec::new(), now };
        self.built(builder, (&request, settling), &delivery).await
    }

    /// Runs the build and settles what it made, giving the job up when either fails.
    async fn built<W: Build>(
        &self,
        builder: &W,
        (request, mut settling): (&Request, Settling),
        delivery: &Delivery,
    ) -> Result<Worked, Failed<P, build::Error<W::Error>>>
    where
        P: 'static,
    {
        let (job, now) = (settling.job.clone(), settling.now);
        match builder.run(request, &mut settling.events).await {
            Err(error) => {
                Self::lost(delivery, &error);
                self.give_up(delivery, &job, now).await?;
                Err(Failed::Other(error))
            }
            Ok(outcome) => match self.settle(settling, outcome).await {
                Ok(worked) => {
                    self.ports.queue().ack(delivery).await.map_err(Error::from)?;
                    Ok(worked)
                }
                Err(error) => {
                    Self::lost(delivery, &error);
                    // a failure to give up is the one returned: the job is still running
                    self.give_up(delivery, &job, now).await?;
                    Err(Failed::Control(error))
                }
            },
        }
    }

    /// What the build of `job` reads: the site's source, the tree a push sent, or the branch a
    /// preview was queued for. A preview of a site that has no git source any more is refused.
    async fn reads(&self, source: build::Source, job: &Job) -> Result<build::Source, Error<P>> {
        Ok(match (source, self.tree_of(job.id).await?, &job.pin) {
            (build::Source::Upload, Some(tree), _) => build::Source::Bundle(tree),
            (build::Source::Git { url, .. }, _, Some(pin)) => {
                build::Source::Git { url, revision: Some(pin.branch.revision()) }
            }
            (_, _, Some(_)) => return Err(Refusal::NoPreview { site: job.site }.into()),
            (source, _, None) => source,
        })
    }

    async fn settle(&self, running: Settling, outcome: Outcome) -> Result<Worked, Error<P>> {
        let Settling { job, tenant, mut events, now } = running;
        let (ended, worked) = match outcome {
            Outcome::Built(built) if job.pin.is_some() => {
                let Some(pin) = job.pin.as_ref() else { return Err(Refusal::NoPreview { site: job.site }.into()) };
                let slug = Slug::branch(pin.branch.as_str()).map_err(Error::Label)?;
                let new = NewPreview { site: job.site, slug: slug.clone(), manifest: built.manifest, at: now };
                self.preview(new).await?;
                (State::Succeeded, Worked::Previewed { build: job.id, slug })
            }
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
        logging::info!(
            name: logging::event::JOB_SETTLED,
            state = ended.name(),
            events = logging::Count::from(events.len()).field(),
        );
        Ok(worked)
    }

    /// The job a delivery is for, marked running; none if the request is not one this plane can
    /// work, in which case [`Control::drop_request`] says why. A job that is neither queued nor
    /// running has ended, and a redelivery does not reopen it.
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
        let outcome = self.ports.db().run(running.into()).await?;
        if outcome.changes != 1 {
            return Ok(None);
        }
        Ok(Some(Job { state: State::Running, ..job }))
    }

    /// Settles a request that cannot be worked, and says why it could not.
    async fn drop_request(&self, delivery: &Delivery) -> Result<Dropped, Error<P>> {
        let dropped = match String::from_utf8(delivery.body.clone()).ok().and_then(|text| text.parse::<BuildId>().ok())
        {
            None => Dropped::Unnamed,
            Some(build) => match self.job(build).await? {
                None => Dropped::NoJob { build },
                Some(job) => match job.state {
                    State::Queued | State::Running => Dropped::NoSite { build, site: job.site },
                    _ => Dropped::Settled { build },
                },
            },
        };
        self.ports.queue().ack(delivery).await?;
        logging::info!(name: logging::event::JOB_DROPPED, reason = logging::Kind::kind(&dropped));
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
/// What the queue gave.
#[derive(Debug)]
enum Taken {
    Idle,
    Dropped(Dropped),
    Job(Box<Claimed>),
}

/// A job that was claimed, with the delivery that carried it.
#[derive(Debug)]
struct Claimed {
    delivery: Delivery,
    job: Job,
    now: Timestamp,
}

#[derive(Debug)]
struct Settling {
    job: Job,
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

/// What a queued job carries besides its row: nothing, the uploaded tree of a push, or the branch
/// and commit of a preview.
#[derive(Clone, Debug)]
enum Attached {
    Nothing,
    Tree(Hash),
    Pin(Box<build::Pin>),
}

impl Attached {
    /// The statement that records it beside the job row, if there is one.
    fn insert(&self, job: BuildId, site: Id<SiteKind>) -> Result<Option<db::Statement>, sea_query::error::Error> {
        Ok(Some(match self {
            Self::Nothing => return Ok(None),
            Self::Tree(tree) => Query::insert()
                .into_table(table::Bundle::Table)
                .columns([table::Bundle::JobId, table::Bundle::SiteId, table::Bundle::Tree])
                .values([Expr::val(job.to_string()), Expr::val(site.to_string()), Expr::val(tree.to_string())])?
                .to_owned()
                .into(),
            Self::Pin(pin) => Query::insert()
                .into_table(table::Pin::Table)
                .columns([table::Pin::JobId, table::Pin::SiteId, table::Pin::Branch, table::Pin::Revision])
                .values([
                    Expr::val(job.to_string()),
                    Expr::val(site.to_string()),
                    Expr::val(pin.branch.as_str()),
                    Expr::val(pin.revision.as_str()),
                ])?
                .to_owned()
                .into(),
        }))
    }
}
