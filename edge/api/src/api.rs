//! The API: one call in, one reply out, over the control plane's ports.

use build::{BuildId, Trigger};
use clock::{Clock, Draw};
use control::{Control, Id, NewJob, NewSite, Ports, Requested, Resumed, Site, SiteKind};
use model::git::Scheme;
use serde::Serialize;
use trace::{Field as _, Instrument as _};

use control::keys::{NewEnrolment, Poll, Target, UserCode, Why};
use db::Db;
use sig::{KeyId, KeyType, Signed};

use crate::Route;
use crate::body::{self, AddSite, Enrolling, Rollback, Unreadable};
use crate::credential::{Credentials, Offered, Presented, Principal, Verdict, Via};
use crate::enrol::Enrol;
use crate::fault::{Denied, Fault};
use crate::limits::Limits;
use crate::reply::{Media, Payload, Reply, Request, Status};
use crate::route::Call;
use crate::view::{
    BuildView, DeployView, Enrolled, GrantView, KeyView, Listed, LiveView, Moved, Polled, Removed, Requested as Asked,
    SiteView, Status as Overview, Whoami,
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
pub struct Api<P: Ports, C, K, D> {
    pub(crate) control: Control<P>,
    credentials: C,
    enrol: Enrol<P::Db>,
    pub(crate) clock: K,
    draw: D,
    pub(crate) limits: Limits,
    schemes: Vec<Scheme>,
}

impl<P: Ports + 'static, C: Credentials, K: Clock, D: Draw> Api<P, C, K, D>
where
    P::Db: Db,
{
    /// The API of `control`, for the callers `credentials` knows, within `limits`.
    pub fn new(control: Control<P>, callers: (C, Enrol<P::Db>), clocks: (K, D), limits: Limits) -> Self {
        let (credentials, enrol) = callers;
        Self { control, credentials, enrol, clock: clocks.0, draw: clocks.1, limits, schemes: vec![Scheme::Https] }
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
        let span = trace::api!(request.method);
        let reply = async {
            match self.answer(request).await {
                Ok(reply) => reply,
                Err(fault) => {
                    trace::warn!(name: trace::event::API_ANSWERED, error = trace::Failure(&fault).field());
                    fault.reply()
                }
            }
        }
        .instrument(span.clone())
        .await;
        span.record("status", trace::Count(u64::from(reply.status.code())).field());
        reply
    }

    async fn answer(&self, request: &Request) -> Result<Reply, Fault<P>> {
        let parsed = Call::parse(request.method, &request.path);
        // asking to enrol is the one call nobody signs: the key has no account yet
        let caller = if matches!(parsed, Ok(Call::Enrol)) { None } else { Some(self.caller(request).await?) };
        let call = parsed.map_err(Fault::Route)?;
        trace::Span::current().record("route", trace::Kind::kind(&call));
        let body = match &request.body {
            Payload::Whole(body) => body.as_slice(),
            Payload::Over { limit } => return Err(Fault::Large { limit: *limit }),
        };
        let Some((caller, signed)) = caller else { return self.enrol(body).await };
        // the digest is checked once the body is read, before anything is done with it
        signed.covers(body).map_err(|_| Fault::Denied(Denied::Digest))?;
        let principal = match (caller, &call) {
            (Caller::Waiting(key), Call::Polled(id)) => return self.polled(*id, key).await,
            (Caller::Known(principal), _) => *principal,
            // a key that is not enrolled may read its own ask and nothing else
            (Caller::Waiting(_), _) => return Err(Fault::Denied(Denied::Refused)),
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
    async fn dispatch(&self, principal: Principal, call: Call, body: &[u8]) -> Result<Reply, Fault<P>> {
        match call {
            Call::Sites => self.sites(principal).await,
            Call::Add => self.add(principal, body).await,
            Call::Status(site) => self.status(principal, site).await,
            Call::Remove(site) => self.remove(principal, site).await,
            Call::Deploys(site) => self.deploys(principal, site).await,
            Call::Rollback(site) => self.rollback(principal, site, body).await,
            Call::Resume(site) => self.resume(principal, site).await,
            Call::Builds(site) => self.builds(principal, site).await,
            Call::Build(site) => self.build(principal, site).await,
            Call::Log { site, build } => self.log(principal, site, build).await,
            Call::Export { site, deploy } => self.export(principal, Route { site, deploy }).await,
            Call::Intake(site) => self.intake(principal, site, body).await,
            Call::Whoami => Self::whoami(&principal),
            Call::Keys => self.keys(&principal).await,
            Call::Revoke(print) => self.revoke(&principal, &print).await,
            Call::Polled(id) => match principal.via.key() {
                Some(key) => self.polled(id, key).await,
                None => Err(Fault::Denied(Denied::Refused)),
            },
            Call::Enrol => Err(Fault::Denied(Denied::Refused)),
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
            .map_err(|error| Fault::Credentials(Box::new(error)));
        // what was decided, and never the header
        let caller = match verdict? {
            Verdict::Granted(principal) => {
                trace::debug!(name: trace::event::API_AUTH, outcome = "granted");
                Caller::Known(Box::new(principal))
            }
            Verdict::Enrolling(key) => {
                trace::debug!(name: trace::event::API_AUTH, outcome = "enrolling");
                Caller::Waiting(key)
            }
            Verdict::Refused(denied) => {
                trace::warn!(name: trace::event::API_AUTH, outcome = trace::Kind::kind(&denied));
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
    fn json<T: Serialize>(status: Status, value: &T) -> Result<Reply, Fault<P>> {
        let body = serde_json::to_vec(value).map_err(|source| Fault::Control(control::Error::Log(source)))?;
        Ok(Reply::new(status, Media::Json, body))
    }

    /// A fresh id.
    fn drawn(&self) -> Result<[u8; 16], Fault<P>> {
        self.draw.bytes().map_err(|error| Fault::Draw(Box::new(error)))
    }

    fn view(&self, site: &Site) -> SiteView {
        SiteView::of(site, self.control.origin().site(&site.handle).ok())
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
        let jobs = self.control.jobs(id, 1).await?;
        let overview = Overview {
            site: self.view(&site),
            live: live.as_ref().map(LiveView::from),
            build: jobs.first().map(BuildView::from),
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
    /// compare. The gate has already turned away a flood.
    async fn enrol(&self, body: &[u8]) -> Result<Reply, Fault<P>> {
        let now = self.clock.now();
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
            None => Fault::Credentials(Box::new(error)),
        }
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

impl<P: Ports + 'static, C: Credentials, K: Clock, D: Draw> Answers for Api<P, C, K, D>
where
    P::Db: Db,
{
    fn answer(&self, request: &Request) -> impl std::future::Future<Output = Reply> {
        self.respond(request)
    }
}
