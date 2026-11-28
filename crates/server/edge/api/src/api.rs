//! The API: one call in, one reply out, over the control plane's ports.

use authority_dns::{Dns, Offline, Resolver};
use authority_forge::Transport;
use authority_repo::{Repos, Silent};
use build::{BuildId, Trigger};
use clock::{Clock, Draw};
use control::{Control, Id, NewJob, NewSite, Ports, Requested, Resumed, Site, SiteKind};
use logging::{Field as _, Instrument as _};
use model::git::Scheme;
use serde::Serialize;

use control::keys::{NewEnrolment, Poll, Target, UserCode, Why};
use db::Db;
use sig::{KeyId, KeyType, Signed};

use crate::Route;
use crate::auth::credential::{Credentials, Offered, Presented, Principal, Verdict, Via};
use crate::auth::signin::{Quota, Rate};
use crate::body::{self, AddSite, Enrolling, Reporting, Rollback, Suspending, Unreadable};
use crate::enrol::{Closed, Enrol};
use crate::fault::{Denied, Fault};
use crate::limits::Limits;
use crate::port::{Port, Unnamed};
use crate::reply::{Media, Payload, Reply, Request, Status};
use crate::route::Call;
use crate::view::{
    BuildView, DeployView, Enrolled, GrantView, KeyView, Listed, LiveView, Moved, Polled, Previewed, Removed,
    ReportView, Reported, Requested as Asked, SiteView, Status as Overview, Suspended, Unpreviewed, Whoami,
};

/// Who a request is, once its signature checked: someone, or a key waiting to be enrolled.
enum Caller {
    Known(Box<Principal>),
    Waiting(KeyId),
}

/// The API over a control plane, a credential check, a clock and a source of ids.
///
/// Nothing here reads the time or draws a random number except through [`Clock`] and [`Draw`],
/// and nothing names an adapter: the same calls on the same ports make the same rows in a test.
#[derive(Debug)]
pub struct Api<P: Ports, C, K, D, R = Offline, T = Silent> {
    pub(crate) control: Control<P>,
    credentials: C,
    enrol: Enrol<P::Db>,
    pub(crate) clock: K,
    pub(crate) draw: D,
    pub(crate) limits: Limits,
    schemes: Vec<Scheme>,
    reports: Quota,
    enrolments: Quota,
    pub(crate) dns: Dns<R>,
    pub(crate) repos: Option<Repos<T>>,
}

impl<P: Ports + 'static, C: Credentials, K: Clock, D: Draw> Api<P, C, K, D>
where
    P::Db: Db,
{
    /// The API of `control`, for the callers `credentials` knows, within `limits`. It has no DNS
    /// resolver, so a custom domain can be claimed and listed and no proof of one can be checked
    /// until [`Api::with_resolver`] gives it one.
    pub fn new(control: Control<P>, callers: (C, Enrol<P::Db>), clocks: (K, D), limits: Limits) -> Self {
        let (credentials, enrol) = callers;
        Self {
            control,
            credentials,
            enrol,
            clock: clocks.0,
            draw: clocks.1,
            limits,
            schemes: vec![Scheme::Https],
            reports: Quota::new(Self::REPORTS, Self::TRACKED),
            enrolments: Quota::new(Self::ENROLMENTS, Self::TRACKED),
            dns: Dns::new(Offline),
            repos: None,
        }
    }
}

impl<P: Ports + 'static, C: Credentials, K: Clock, D: Draw, R: Resolver, T: Transport> Api<P, C, K, D, R, T>
where
    P::Db: Db,
{
    /// The same API, reading the proof records of custom domains through `resolver`.
    pub fn with_resolver<S: Resolver>(self, resolver: S) -> Api<P, C, K, D, S, T> {
        Api {
            control: self.control,
            credentials: self.credentials,
            enrol: self.enrol,
            clock: self.clock,
            draw: self.draw,
            limits: self.limits,
            schemes: self.schemes,
            reports: self.reports,
            enrolments: self.enrolments,
            dns: Dns::new(resolver),
            repos: self.repos,
        }
    }

    /// The same API, reaching the forges `repos` knows to prove a site's repository. With `None` a
    /// repository can be listed and nothing can be claimed.
    pub fn with_repos<S: Transport>(self, repos: Option<Repos<S>>) -> Api<P, C, K, D, R, S> {
        Api {
            control: self.control,
            credentials: self.credentials,
            enrol: self.enrol,
            clock: self.clock,
            draw: self.draw,
            limits: self.limits,
            schemes: self.schemes,
            reports: self.reports,
            enrolments: self.enrolments,
            dns: self.dns,
            repos,
        }
    }

    /// What asking to enrol is held to by default: five an hour from one source. It is charged
    /// before the global gate, so one source cannot fill the queue of waiting keys and close
    /// enrolment for everybody; it is under the gate's default of 20 waiting keys for that reason.
    pub const ENROLMENTS: Rate = Rate { count: 5, window: control::signin::Ttl::secs(3600) };

    /// What abuse reports are held to by default: five an hour from one source (decision 0089).
    /// A report costs a row and names a stranger's site, so the limit bounds rows and noise.
    pub const REPORTS: Rate = Rate { count: 5, window: control::signin::Ttl::secs(3600) };

    /// How many sources the report limit remembers at once.
    const TRACKED: usize = 10_000;

    /// The same API, taking abuse reports at `rate` for each source.
    #[must_use]
    pub fn with_reports(mut self, rate: Rate) -> Self {
        self.reports = Quota::new(rate, Self::TRACKED);
        self
    }

    /// The same API, taking asks to enrol at `rate` for each source.
    #[must_use]
    pub fn with_enrolments(mut self, rate: Rate) -> Self {
        self.enrolments = Quota::new(rate, Self::TRACKED);
        self
    }

    /// The same API, taking git sources over `schemes` only. A new API takes `https`; a server
    /// that builds from its own disk adds `file`.
    #[must_use]
    pub fn with_schemes(mut self, schemes: Vec<Scheme>) -> Self {
        self.schemes = schemes;
        self
    }

    /// The control plane the API answers from.
    pub const fn control(&self) -> &Control<P> {
        &self.control
    }

    /// The limits.
    pub const fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Answers `request`. Never fails: a failure is a reply that says what went wrong.
    pub async fn respond(&self, request: &Request) -> Reply {
        let span = logging::api!(request.method);
        let reply = async {
            match self.answer(request).await {
                Ok(reply) => reply,
                Err(fault) => {
                    logging::warn!(name: logging::event::API_ANSWERED, error = logging::Failure(&fault).field());
                    fault.reply()
                }
            }
        }
        .instrument(span.clone())
        .await;
        span.record("status", logging::Count(u64::from(reply.status.code())).field());
        reply
    }

    async fn answer(&self, request: &Request) -> Result<Reply, Fault<P>> {
        let parsed = Call::parse(request.method, &request.path);
        // asking to enrol is the one call nobody signs: the key has no account yet
        let caller =
            if matches!(parsed, Ok(Call::Enrol | Call::Report)) { None } else { Some(self.caller(request).await?) };
        let call = parsed.map_err(Fault::Route)?;
        logging::Span::current().record("route", logging::Kind::kind(&call));
        let body = match &request.body {
            Payload::Whole(body) => body.as_slice(),
            Payload::Over { limit } => return Err(Fault::Large { limit: *limit }),
        };
        let Some((caller, signed)) = caller else {
            return match call {
                Call::Report => self.report(request.source, body).await,
                _ => self.enrol(request.source, body).await,
            };
        };
        // the digest is checked once the body is read, before anything is done with it
        signed.covers(body).map_err(|_| Fault::Denied(Denied::Digest))?;
        let principal = match (caller, &call) {
            (Caller::Waiting(key), Call::Polled(id)) => return self.polled(*id, key).await,
            (Caller::Known(principal), _) => *principal,
            // a key that is not enrolled may read its own ask and nothing else
            (Caller::Waiting(_), _) => return Err(Fault::Denied(Denied::Refused)),
        };
        if call.operator() && !matches!(principal.via, Via::Operator { .. }) {
            return Err(Fault::Denied(Denied::Operator));
        }
        // the call may be made as an organization: the account, the key's reach and the owner a
        // site is checked against all become the organization's together
        let principal = match Call::acting(&request.path).or(call.org()) {
            Some(org) => self.acting(principal, org).await?,
            None => principal,
        };
        if let Some((right, site)) = call.needs() {
            let target = Target { account: principal.account, site };
            if !principal.effective.allows(right, &target) {
                let scope = if site.is_some() { "site" } else { "account" };
                return Err(Fault::Denied(Denied::Right { right, scope }));
            }
        }
        self.dispatch(principal, call, body).await
    }

    /// The call, for a caller who may make it.
    #[expect(clippy::too_many_lines, reason = "one arm for each call")]
    async fn dispatch(&self, principal: Principal, call: Call, body: &[u8]) -> Result<Reply, Fault<P>> {
        match call {
            Call::Sites => self.sites(principal).await,
            Call::Add => self.add(principal, body).await,
            Call::Status(site) => self.status(principal, site).await,
            Call::Remove(site) => self.remove(principal, site).await,
            Call::Deploys(site) => self.deploys(principal, site).await,
            Call::Rollback(site) => self.rollback(principal, site, body).await,
            Call::Resume(site) => self.resume(principal, site).await,
            Call::Reports => self.reports().await,
            Call::Suspend(site) => self.suspend(principal, site, body).await,
            Call::Reinstate(site) => self.reinstate(site).await,
            Call::Builds(site) => self.builds(principal, site).await,
            Call::Build(site) => self.build(principal, site).await,
            Call::Log { site, build } => self.log(principal, site, build).await,
            Call::Export { site, deploy } => self.export(principal, Route { site, deploy }).await,
            Call::Intake(site) => self.intake(principal, site, body).await,
            Call::Domains(site) => self.domains(principal, site).await,
            Call::Claim { site, name } => self.claim(principal, (site, name)).await,
            Call::Prove { site, name } => self.prove(principal, (site, name)).await,
            Call::Unclaim { site, name } => self.unclaim(principal, (site, name)).await,
            Call::Repos(site) => self.repo_list(principal, site).await,
            Call::Repo { site, step } => self.repo_step(principal, (site, step), body).await,
            Call::Previews(site) => self.previews(principal, site).await,
            Call::Preview { site, slug } => self.preview(principal, (site, slug), body).await,
            Call::Unpreview { site, slug } => self.unpreview(principal, (site, slug)).await,
            Call::Source(site) => self.probe(principal, site, body).await,
            Call::Parts(site) => self.parts(principal, site, body).await,
            Call::Bundle(site) => self.bundle(principal, site).await,
            Call::Whoami => Self::whoami(&principal),
            Call::Keys if principal.account != principal.person => self.org_keys(&principal).await,
            Call::Keys => self.keys(&principal).await,
            Call::Orgs
            | Call::OrgAdd
            | Call::OrgShow(_)
            | Call::MemberAdd(_)
            | Call::MemberRole { .. }
            | Call::MemberOut { .. } => self.members(&principal, call, body).await,
            Call::Revoke(print) => self.revoke(&principal, &print).await,
            Call::Polled(id) => match principal.via.key() {
                Some(key) => self.polled(id, key).await,
                None => Err(Fault::Denied(Denied::Refused)),
            },
            Call::Enrol | Call::Report => Err(Fault::Denied(Denied::Refused)),
        }
    }

    /// Who is calling, or why nobody is; and what they signed, to check the body against.
    async fn caller(&self, request: &Request) -> Result<(Caller, Signed), Fault<P>> {
        let header = request.authorization.as_deref().ok_or(Fault::Denied(Denied::Missing))?;
        let signed = Signed::parse(header).map_err(|_| Fault::Denied(Denied::Malformed))?;
        let target = request.target().map_err(|_| Fault::Denied(Denied::NotCanonical))?;
        let method = sig::Method::parse(&request.method.to_string()).map_err(|_| Fault::Denied(Denied::Malformed))?;
        let presented = Presented { method: &method, target: &target };
        let verdict = self
            .credentials
            .verify(&Offered::Signed(signed), &presented)
            .await
            .map_err(|error| Fault::Port(Unnamed::new(Port::Credentials, error)));
        // what was decided, and never the header
        let caller = match verdict? {
            Verdict::Granted(principal) => {
                logging::debug!(name: logging::event::API_AUTH, outcome = "granted");
                Caller::Known(Box::new(principal))
            }
            Verdict::Enrolling(key) => {
                logging::debug!(name: logging::event::API_AUTH, outcome = "enrolling");
                Caller::Waiting(key)
            }
            Verdict::Refused(denied) => {
                logging::warn!(name: logging::event::API_AUTH, outcome = logging::Kind::kind(&denied));
                return Err(Fault::Denied(denied));
            }
        };
        Ok((caller, signed))
    }

    /// The site, which must be the caller's: another account's site is no site.
    pub(crate) async fn owned(&self, principal: Principal, id: Id<SiteKind>) -> Result<Site, Fault<P>> {
        let site = self.control.site(id).await?;
        if site.account == principal.account { Ok(site) } else { Err(control::Refusal::NoSite { site: id }.into()) }
    }

    /// A JSON reply.
    pub(crate) fn json<V: Serialize>(status: Status, value: &V) -> Result<Reply, Fault<P>> {
        let body = serde_json::to_vec(value).map_err(|source| Fault::Control(control::Error::Log(source)))?;
        Ok(Reply::new(status, Media::Json, body))
    }

    /// A fresh id.
    pub(crate) fn drawn(&self) -> Result<[u8; 16], Fault<P>> {
        self.draw.bytes().map_err(|error| Fault::Port(Unnamed::new(Port::Draw, error)))
    }

    fn view(&self, site: &Site) -> SiteView {
        SiteView::of(site, Self::linked(self.control.origin().site(&site.handle)))
    }

    /// The address `built` made, or none, told as an event when it could not be built.
    pub(crate) fn linked(built: Result<model::ExternalUrl, model::UrlError>) -> Option<model::ExternalUrl> {
        match built {
            Ok(url) => Some(url),
            Err(error) => {
                logging::warn!(name: logging::event::API_UNLINKED, error = logging::Failure(&error).field());
                None
            }
        }
    }

    async fn sites(&self, principal: Principal) -> Result<Reply, Fault<P>> {
        let sites = self.control.sites(principal.account).await?;
        Self::json(Status::Ok, &Listed { items: sites.iter().map(|site| self.view(site)).collect() })
    }

    async fn add(&self, principal: Principal, body: &[u8]) -> Result<Reply, Fault<P>> {
        let new: AddSite = Unreadable::read(body).map_err(Fault::Body)?;
        if let body::Origin::Git { url, .. } = &new.source
            && !self.schemes.contains(&url.0.scheme())
        {
            return Err(Fault::Scheme { scheme: url.0.scheme(), allowed: self.schemes.clone() });
        }
        let new = NewSite {
            id: Id::from_bytes(self.drawn()?),
            account: principal.account,
            handle: new.handle.0,
            source: new.source.into(),
            at: self.clock.now(),
        };
        let site = self.control.add_site(new).await?;
        Self::json(Status::Created, &self.view(&site))
    }

    async fn status(&self, principal: Principal, id: Id<SiteKind>) -> Result<Reply, Fault<P>> {
        let site = self.owned(principal, id).await?;
        let live = self.control.live(id).await?;
        let jobs = self.control.jobs(id, self.limits.rows.max(1)).await?;
        let waiting = |job: &&control::Job| matches!(job.state, control::JobState::Queued | control::JobState::Running);
        let overview = Overview {
            site: self.view(&site),
            live: live.as_ref().map(LiveView::from),
            build: jobs.first().map(BuildView::from),
            pending: jobs.iter().filter(waiting).map(BuildView::from).collect(),
        };
        Self::json(Status::Ok, &overview)
    }

    async fn remove(&self, principal: Principal, id: Id<SiteKind>) -> Result<Reply, Fault<P>> {
        self.owned(principal, id).await?;
        let receipt = self.control.remove_site(id, self.clock.now()).await?;
        Self::json(Status::Ok, &Removed::from(&receipt))
    }

    async fn deploys(&self, principal: Principal, id: Id<SiteKind>) -> Result<Reply, Fault<P>> {
        self.owned(principal, id).await?;
        let deploys = self.control.deploys(id, self.limits.rows).await?;
        Self::json(Status::Ok, &Listed { items: deploys.iter().map(DeployView::from).collect() })
    }

    async fn rollback(&self, principal: Principal, id: Id<SiteKind>, body: &[u8]) -> Result<Reply, Fault<P>> {
        self.owned(principal, id).await?;
        let to: Rollback = Unreadable::read(body).map_err(Fault::Body)?;
        let live = self.control.rollback(id, to.deploy, self.clock.now()).await?;
        Self::json(Status::Ok, &Moved { live: Some(LiveView::from(&live)), was_paused: None })
    }

    async fn resume(&self, principal: Principal, id: Id<SiteKind>) -> Result<Reply, Fault<P>> {
        self.owned(principal, id).await?;
        let resumed = self.control.resume(id, self.clock.now()).await?;
        let live = self.control.live(id).await?;
        let was_paused = Some(resumed == Resumed::Resumed);
        Self::json(Status::Ok, &Moved { live: live.as_ref().map(LiveView::from), was_paused })
    }

    async fn builds(&self, principal: Principal, id: Id<SiteKind>) -> Result<Reply, Fault<P>> {
        self.owned(principal, id).await?;
        let jobs = self.control.jobs(id, self.limits.rows).await?;
        Self::json(Status::Ok, &Listed { items: jobs.iter().map(BuildView::from).collect() })
    }

    async fn build(&self, principal: Principal, id: Id<SiteKind>) -> Result<Reply, Fault<P>> {
        let site = self.owned(principal, id).await?;
        if site.source == build::Source::Upload {
            return Err(Fault::Unbuilt);
        }
        let new = NewJob {
            id: BuildId::from_bytes(self.drawn()?),
            site: id,
            trigger: Trigger::Manual,
            dedupe: None,
            at: self.clock.now(),
        };
        match self.control.request_build(new).await? {
            Requested::Queued(build) => Self::json(Status::Created, &Asked::Queued { build }),
            _ => Self::json(Status::Ok, &Asked::Duplicate),
        }
    }

    async fn log(&self, principal: Principal, id: Id<SiteKind>, build: BuildId) -> Result<Reply, Fault<P>> {
        self.owned(principal, id).await?;
        let log = self.control.build_log(id, build).await?.ok_or(Fault::NoLog)?;
        Ok(Reply::new(Status::Ok, Media::Ndjson, log))
    }

    /// `POST /v1/enrol`: an unsigned ask, answered with the code to type and the fingerprint to
    /// compare. The source's limit and then the gate turn away a flood.
    async fn enrol(&self, source: Option<std::net::IpAddr>, body: &[u8]) -> Result<Reply, Fault<P>> {
        let now = self.clock.now();
        // the source's own limit comes first, so what a flood from one source spends is its own
        self.enrolments.admit(source, now).map_err(|limited| Closed { seconds: limited.retry.as_secs() })?;
        self.enrol.gate().open(now)?;
        let waiting = self.enrol.keys().waiting(now).await.map_err(Self::keys_failed)?;
        self.enrol.gate().full(waiting, now)?;
        let asked: Enrolling = Unreadable::read(body).map_err(Fault::Body)?;
        let new = NewEnrolment {
            id: Id::from_bytes(self.drawn()?),
            code: UserCode::drawn(self.drawn()?),
            key_type: KeyType::Ed25519V1,
            public: asked.key.0,
            label: asked.label.0,
            purpose: asked.purpose.0,
            site: asked.site.map(|site| site.0),
            rights: asked.rights.map_or(control::keys::Rights::NONE, |rights| rights.0),
            lifetime: asked.lifetime.map(|lifetime| lifetime.0),
            at: now,
        };
        let key = new.public.id(new.key_type);
        let made = self.enrol.keys().enrol(new).await.map_err(Self::keys_failed)?;
        let shown = Enrolled {
            id: made.id,
            user_code: made.code.to_string(),
            fingerprint: key.fingerprint().to_string(),
            expires: made.expires,
        };
        Self::json(Status::Created, &shown)
    }

    /// `POST /v1/abuse`: takes a report from anyone, within the per-source limit.
    async fn report(&self, source: Option<std::net::IpAddr>, body: &[u8]) -> Result<Reply, Fault<P>> {
        let now = self.clock.now();
        self.reports
            .admit(source, now)
            .map_err(|limited| Fault::Reports(Closed { seconds: limited.retry.as_secs() }))?;
        let said: Reporting = Unreadable::read(body).map_err(Fault::Body)?;
        let new = control::NewReport {
            id: Id::from_bytes(self.drawn()?),
            handle: said.handle.0,
            kind: said.kind,
            detail: said.detail,
            contact: said.contact,
            at: now,
        };
        let kept = self.control.report(new).await?;
        Self::json(Status::Created, &Reported { id: kept.id })
    }

    /// `GET /v1/abuse`: the reports, newest first, for the operator.
    async fn reports(&self) -> Result<Reply, Fault<P>> {
        let reports = self.control.reports(u64::from(self.limits.rows)).await?;
        Self::json(Status::Ok, &Listed { items: reports.iter().map(ReportView::from).collect() })
    }

    /// `POST /v1/sites/{site}/suspend`: the operator takes the site down.
    async fn suspend(&self, principal: Principal, site: Id<SiteKind>, body: &[u8]) -> Result<Reply, Fault<P>> {
        let said: Suspending = Unreadable::read(body).map_err(Fault::Body)?;
        let by = match principal.via {
            Via::Operator { label } | Via::Key { label, .. } => label,
        };
        let suspension = control::Suspension { site, kind: said.kind, by, report: said.report, at: self.clock.now() };
        let made = self.control.suspend(suspension).await?;
        Self::json(Status::Ok, &Suspended { site, kind: Some(made.kind), at: made.at })
    }

    /// `POST /v1/sites/{site}/reinstate`: the operator lifts the suspension.
    async fn reinstate(&self, site: Id<SiteKind>) -> Result<Reply, Fault<P>> {
        let at = self.clock.now();
        self.control.reinstate(site, at).await?;
        Self::json(Status::Ok, &Suspended { site, kind: None, at })
    }

    /// `GET /v1/enrol/{id}`: the ask's outcome, to the key that made it.
    async fn polled(&self, id: Id<control::EnrolmentKind>, key: KeyId) -> Result<Reply, Fault<P>> {
        let found = self.enrol.keys().poll(id, self.clock.now()).await.map_err(Self::keys_failed)?;
        // another key's ask is no ask: the answer for an id nobody made
        let (_, poll) = found.filter(|(ask, _)| ask.key() == key).ok_or(control::keys::Refused::NoEnrolment)?;
        match poll {
            Poll::Pending => Self::json(Status::Ok, &Polled { state: "pending", expires: None }),
            Poll::Approved { expires } => Self::json(Status::Ok, &Polled { state: "approved", expires: Some(expires) }),
            Poll::Denied => Err(Fault::Declined),
            _ => Err(Fault::Keys(control::keys::Refused::NoEnrolment)),
        }
    }

    fn whoami(principal: &Principal) -> Result<Reply, Fault<P>> {
        let (kind, fingerprint, label) = match &principal.via {
            Via::Operator { label } => ("operator", None, label.clone()),
            Via::Key { id, kind, label } => (kind.name(), Some(id.fingerprint().to_string()), label.clone()),
        };
        let view = Whoami {
            account: principal.account,
            kind,
            fingerprint,
            label,
            grants: principal.effective.grants().list().iter().map(GrantView::from).collect(),
            expires: principal.expires,
        };
        Self::json(Status::Ok, &view)
    }

    async fn keys(&self, principal: &Principal) -> Result<Reply, Fault<P>> {
        let keys = self.enrol.keys().list(principal.account).await.map_err(Self::keys_failed)?;
        Self::json(Status::Ok, &Listed { items: keys.iter().map(KeyView::from).collect() })
    }

    async fn revoke(&self, principal: &Principal, print: &sig::Fingerprint) -> Result<Reply, Fault<P>> {
        let own = principal.via.key().map(|key| key.fingerprint());
        let why = if own.as_ref() == Some(print) { Why::LoggedOut } else { Why::Revoked };
        let key = self
            .enrol
            .keys()
            .revoke((principal.account, print), why, self.clock.now())
            .await
            .map_err(Self::keys_failed)?;
        Self::json(Status::Ok, &KeyView::from(&key))
    }

    /// A refusal of the key store is the caller's; anything else is the server's.
    fn keys_failed(error: control::keys::Error<<P::Db as Db>::Error>) -> Fault<P> {
        match error.refusal() {
            Some(refused) => Fault::Keys(refused.clone()),
            None => Fault::Store(error),
        }
    }

    async fn previews(&self, principal: Principal, id: Id<SiteKind>) -> Result<Reply, Fault<P>> {
        let site = self.owned(principal, id).await?;
        let listed = self.control.previews(id, self.clock.now()).await?;
        let origin = self.control.origin();
        let items =
            listed.iter().map(|found| Previewed::of(found, Self::linked(origin.previewed(&found.slug, &site.handle))));
        Self::json(Status::Ok, &Listed { items: items.collect() })
    }

    async fn preview(
        &self,
        principal: Principal,
        target: (Id<SiteKind>, control::Slug),
        body: &[u8],
    ) -> Result<Reply, Fault<P>> {
        let (id, slug) = target;
        let site = self.owned(principal, id).await?;
        let made = self.previewing(&site, slug, body).await?;
        Self::json(Status::Created, &made)
    }

    /// `DELETE /v1/sites/{site}/previews/{label}`: the preview ends now, with the files only it held.
    async fn unpreview(&self, principal: Principal, target: (Id<SiteKind>, control::Slug)) -> Result<Reply, Fault<P>> {
        let (id, slug) = target;
        self.owned(principal, id).await?;
        let swept = self.control.remove_preview(id, &slug, self.clock.now()).await?;
        Self::json(Status::Ok, &Unpreviewed { slug: slug.to_string(), blobs: swept.blobs })
    }

    async fn intake(&self, principal: Principal, id: Id<SiteKind>, body: &[u8]) -> Result<Reply, Fault<P>> {
        let site = self.owned(principal, id).await?;
        let deployed = self.deploy(&site, body).await?;
        Self::json(Status::Created, &deployed)
    }
}

/// What an adapter needs of an API: an answer, and what a body may cost. [`Api`] is the one
/// implementation; the adapter's bridge is generic over this so it names no port.
pub trait Answers {
    /// Answers `request`.
    fn answer(&self, request: &Request) -> impl std::future::Future<Output = Reply>;
}

impl<P: Ports + 'static, C: Credentials, K: Clock, D: Draw, R: Resolver, T: Transport> Answers for Api<P, C, K, D, R, T>
where
    P::Db: Db,
{
    fn answer(&self, request: &Request) -> impl std::future::Future<Output = Reply> {
        self.respond(request)
    }
}
