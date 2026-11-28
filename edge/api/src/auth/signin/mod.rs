//! Signing in over the API (decision 0066): the routes under `/v1/login` and `/v1/session`.
//!
//! A [`Door`] answers them over [`control::signin::SignIn`], and [`Routed`] puts it beside the
//! [`crate::Api`] so one `Answers` takes every request. The web screens (the page a link opens,
//! the form that asks for one) are the `web` branch's: they call these routes, and the link a
//! message carries points at whatever base the operator gives (`Config::base`).
//!
//! | Route | What it does |
//! |---|---|
//! | `POST /v1/login` `{"address"}` | Asks for a link. Always answers `202 {"requested":true}` in the same shape, whether or not the address has an account, whether or not mail will be sent. `429` with `Retry-After` when a rate limit is reached; that too is independent of whether the address exists. |
//! | `GET /v1/login/{token}` | What the link is for and when it ends, without using it, so a mail scanner that opens links does not spend it. `404` for any token that is no good, in one shape. With `reveals` it signs in as `POST` does. |
//! | `POST /v1/login/{token}` | Uses the link. `200` with the session secret in the body (a bearer token) and in `Set-Cookie`. The first-run link also takes `{"address"}` to give the owner's account. |
//! | `GET /v1/session` | The session of the bearer token or the cookie. `401` otherwise. |
//! | `DELETE /v1/session` | Ends it and clears the cookie. |
//! | `GET /v1/sessions` | The account's live sessions, newest first, with the asking one marked `current`. No secret is in it. |
//! | `DELETE /v1/sessions/{id}` | Ends one of the account's sessions by its id. `404` for an id that is another account's, ended or never made. Ending the asking one clears the cookie. |
//!
//! **Enumeration.** The answer to `POST /v1/login` is one value of one type, built before the
//! outcome is known. The work differs by one local row write, and no mail is sent in the request:
//! it goes to the [`Outbox`], so the slow part (SMTP) is not in the request's time. What is left
//! is a database insert against none, well under the jitter of a network, and the rate limits
//! count every request alike, so they leak nothing either. This is flat enough against a
//! remote observer, and it is not constant time against one on the same machine.
//!
//! **Origin.** Every route here that changes state, which is every one but a `GET`, is taken from
//! the origin of the link base (and any added with [`Door::with_origin`]) or with
//! `Sec-Fetch-Site: same-origin`, and refused otherwise as `auth.origin.refused`. A script that is
//! no browser signs its requests on the API instead.
//!
//! **Tokens in logs.** A link's token is in a path. [`Enter::redact`] cuts it from every line the
//! server logs, [`Request`]'s `Debug` does the same, and no error of this module holds it.

mod approver;
mod limiter;
mod outbox;
mod route;

use std::time::Duration;

use clock::{Clock, Draw};
use control::signin::{Drawn, Error, Presented, Proof, Refused, Requested, Session, SignIn};
use db::Db;
use jiff::Timestamp;
use mail::{Message, Template};
use serde::Serialize;

use crate::Answers;
use crate::auth::cookie::Jar;
use crate::auth::origin::Origins;
use crate::body::{Approve, Ask, Redeem, Unreadable};
use crate::enrol::{Closed, Enrol};
use crate::port::{Port, Unnamed};
use crate::refusal::Turned;
use crate::reply::{Media, Method, Payload, Reply, Request, Status};
use crate::view::Problem;
use control::keys::{Approval, Error as KeyError, Refused as KeyRefused, Rights};

pub use approver::{Approver, Approving, Strength};
pub use limiter::{Limited, Limiter, Quota, Rate, RateError, Rates, Scope, Who};
pub use outbox::{Courier, Full, Note, Notes, Outbox};
pub use route::{Enter, EnterError};

/// How a [`Door`] is set up. Every field is the operator's.
#[derive(Debug)]
pub struct Config {
    /// The session cookie.
    pub jar: Jar,
    /// Where the link in a message points; the token is its last segment.
    pub base: mail::Base,
    /// The limits on asking for links.
    pub rates: Rates,
    /// Where messages wait for the courier.
    pub outbox: Outbox,
    /// Whether `GET /v1/login/{token}` signs in. Off, it only looks, because a scanner that
    /// opens every link in a mail would spend the link before its owner clicks.
    pub reveals: bool,
}

/// The sign-in routes over a database, a clock and a source of randomness.
#[derive(Debug)]
pub struct Door<D, K, R> {
    entry: SignIn<D>,
    enrol: Enrol<D>,
    clock: K,
    draw: R,
    config: Config,
    limiter: Limiter,
    origins: Origins,
}

/// Why a sign-in call was not answered with what it asked for.
#[derive(Debug)]
#[non_exhaustive]
pub enum Step<E> {
    /// The path is no sign-in route.
    Route,
    /// The method is not taken there.
    Method(EnterError),
    /// The body is not the JSON the route reads.
    Body(Unreadable),
    /// The body went over its limit.
    Large {
        /// The limit.
        limit: u64,
    },
    /// A rate limit was reached.
    Limited(Limited),
    /// Enrolling is closed for a while.
    Closed(Closed),
    /// The key store refused.
    Keys(KeyRefused),
    /// The key store failed.
    Store(KeyError<E>),
    /// A state-changing request that did not come from one of the dashboard's origins.
    Origin,
    /// The request holds no usable session.
    Anonymous,
    /// The account has no live session with that id.
    Missing,
    /// The random source gave nothing.
    Draw(Unnamed),
    /// Signing in refused or failed.
    Sign(Error<E>),
    /// The answer could not be written as JSON.
    Json(serde_json::Error),
}

impl<E> std::fmt::Display for Step<E> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Route => out.write_str("there is no such route"),
            Self::Method(error) => error.fmt(out),
            Self::Body(error) => error.fmt(out),
            Self::Large { limit } => write!(out, "the body is over the limit of {limit} bytes"),
            Self::Limited(error) => error.fmt(out),
            Self::Closed(error) => error.fmt(out),
            Self::Keys(error) => error.fmt(out),
            Self::Store(_) => out.write_str("the keys could not be read"),
            Self::Origin => out.write_str("the request did not come from this dashboard"),
            Self::Anonymous => out.write_str("sign in on the dashboard: this route reads the session cookie"),
            Self::Missing => out.write_str("the account has no live session with that id"),
            Self::Draw(_) => out.write_str("no random bytes were available"),
            Self::Sign(error) => error.fmt(out),
            Self::Json(_) => out.write_str("the answer could not be written"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Step<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Method(source) => Some(source),
            Self::Body(source) => Some(source),
            Self::Limited(source) => Some(source),
            Self::Closed(source) => Some(source),
            Self::Keys(source) => Some(source),
            Self::Store(source) => Some(source),
            Self::Draw(source) => Some(source),
            Self::Sign(source) => Some(source),
            Self::Json(source) => Some(source),
            Self::Route | Self::Large { .. } | Self::Origin | Self::Anonymous | Self::Missing => None,
        }
    }
}

impl<E: std::error::Error + 'static> Step<E> {
    /// A stable name for what went wrong.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Route | Self::Method(_) => "route",
            Self::Body(_) => "body",
            Self::Large { .. } => "too-large",
            Self::Limited(_) => "rate-limited",
            Self::Closed(_) => "auth.enrolment.limited",
            Self::Keys(refused) => Turned(refused).code(),
            Self::Store(_) => "keys",
            Self::Origin => "auth.origin.refused",
            Self::Anonymous => "unauthorized",
            Self::Missing => "auth.session.missing",
            Self::Draw(_) => "random",
            Self::Json(_) => "json",
            Self::Sign(error) => match error {
                Error::Refused(Refused::Unusable) => "login",
                Error::Refused(_) => "refused",
                _ => "signin",
            },
        }
    }

    /// The status it is answered with. A link that cannot be used is a `404`, whatever the
    /// reason, so the answer is one shape.
    pub fn status(&self) -> Status {
        match self {
            Self::Route | Self::Missing | Self::Sign(Error::Refused(Refused::Unusable)) => Status::NotFound,
            Self::Method(_) => Status::MethodNotAllowed,
            Self::Body(_) => Status::BadRequest,
            Self::Large { .. } => Status::TooLarge,
            Self::Limited(_) | Self::Closed(_) => Status::TooMany,
            Self::Keys(refused) => Turned(refused).status(),
            Self::Origin => Status::Forbidden,
            Self::Anonymous => Status::Unauthorized,
            Self::Sign(Error::Refused(_)) => Status::Conflict,
            Self::Draw(_) | Self::Json(_) | Self::Sign(_) | Self::Store(_) => Status::Internal,
        }
    }

    /// The answer: a status and a [`Problem`]. A failure of the server's own gives its message
    /// and no causes.
    pub fn reply(&self) -> Reply {
        let status = self.status();
        let mut causes = Vec::new();
        let mut below = std::error::Error::source(self).filter(|_| status != Status::Internal);
        while let Some(cause) = below {
            causes.push(cause.to_string());
            below = cause.source();
        }
        let problem = Problem {
            problem: self.name(),
            message: self.to_string(),
            causes,
            fields: std::collections::BTreeMap::default(),
        };
        let mut reply = Reply::new(status, Media::Json, serde_json::to_vec(&problem).unwrap_or_default());
        reply.challenge = matches!(self, Self::Anonymous);
        match self {
            Self::Method(error) => reply.allow.clone_from(&error.allowed),
            Self::Limited(limited) => reply.retry = Some(limited.retry),
            Self::Closed(closed) => reply.retry = Some(Duration::from_secs(closed.seconds)),
            _ => {}
        }
        reply
    }
}

/// Draws what one sign-in call needs from a [`Draw`]: five draws of 16 bytes.
#[derive(Debug)]
pub struct Draws<'a, R>(pub &'a R);

impl<R: Draw> Draws<'_, R> {
    /// A [`Drawn`] from five draws.
    pub fn drawn(&self) -> Result<Drawn, R::Error> {
        let mut bytes = [[0_u8; 16]; 5];
        for slot in &mut bytes {
            *slot = self.0.bytes()?;
        }
        Ok(Drawn::new(bytes))
    }
}

/// What `POST /v1/login` always answers.
#[derive(Debug, Serialize)]
struct Asked {
    requested: bool,
}

/// What a link is for.
#[derive(Debug, Serialize)]
struct Looked {
    purpose: &'static str,
    expires: Timestamp,
}

/// A session, without its secret.
#[derive(Debug, Serialize)]
struct Shown {
    id: control::Id<control::SessionKind>,
    account: control::Id<control::AccountKind>,
    created: Timestamp,
    seen: Timestamp,
    expires: Timestamp,
    limit: Timestamp,
}

impl From<&Session> for Shown {
    fn from(session: &Session) -> Self {
        Self {
            id: session.id,
            account: session.account,
            created: session.created,
            seen: session.seen,
            expires: session.expires,
            limit: session.limit,
        }
    }
}

/// A sign-in: the session's secret, once. It is a bearer token and the value of the cookie.
#[derive(Serialize)]
struct Signed<'a> {
    session: &'a str,
    #[serde(flatten)]
    shown: Shown,
    created_account: bool,
}

/// A session ended.
#[derive(Debug, Serialize)]
struct Ended {
    ended: bool,
}

/// A session of the account's list: what [`Shown`] says, and whether it is the one asking.
#[derive(Debug, Serialize)]
struct Listed {
    #[serde(flatten)]
    shown: Shown,
    current: bool,
}

impl<E> Step<E> {
    /// The refusal for a link that cannot be used, which is one answer for every reason.
    const fn unusable() -> Self {
        Self::Sign(Error::Refused(Refused::Unusable))
    }
}

impl<D: Db, K: Clock, R: Draw> Door<D, K, R> {
    /// The sign-in routes over `entry`, with the time from `clocks.0` and randomness from
    /// `clocks.1`, and the approval of enrolments over `enrol`.
    pub fn new(entry: SignIn<D>, clocks: (K, R), enrol: Enrol<D>, config: Config) -> Self {
        let origins = Origins::of(&config.base);
        Self { entry, enrol, clock: clocks.0, draw: clocks.1, config, limiter: Limiter::new(), origins }
    }

    /// The same door also taking state-changing requests from `origin`, such as a dashboard served
    /// on another host than the link base. A request is taken from the link base's origin, and
    /// from these, and from nowhere else.
    #[must_use]
    pub fn with_origin(mut self, origin: impl Into<String>) -> Self {
        self.origins = self.origins.with(origin);
        self
    }

    /// The origins a state-changing request may come from.
    pub const fn origins(&self) -> &Origins {
        &self.origins
    }

    /// How it is set up.
    pub const fn config(&self) -> &Config {
        &self.config
    }

    /// Answers `request`. Never fails: a failure is a reply that says what went wrong.
    pub async fn respond(&self, request: &Request) -> Reply {
        match self.answer(request).await {
            Ok(reply) => reply,
            Err(step) => {
                let mut reply = step.reply();
                if matches!(step, Step::Anonymous) && request.cookie.is_some() {
                    reply.cookie = Some(self.config.jar.clear());
                }
                reply
            }
        }
    }

    async fn answer(&self, request: &Request) -> Result<Reply, Step<D::Error>> {
        let call = Enter::parse(request.method, &request.path).ok_or(Step::Route)?.map_err(Step::Method)?;
        let body = match &request.body {
            Payload::Whole(body) => body.as_slice(),
            Payload::Over { limit } => return Err(Step::Large { limit: *limit }),
        };
        // a `GET` that signs in (`reveals`) is the operator's choice for an instance with no page
        // in front, and has no browser to ask: everything else that changes state is checked
        if request.method != Method::Get {
            self.origin(request)?;
        }
        match call {
            Enter::Ask => self.ask(request, body).await,
            Enter::Look(proof) if !self.config.reveals => self.look(proof).await,
            Enter::Look(proof) | Enter::Redeem(proof) => self.redeem(proof, body).await,
            Enter::Show => self.show(request).await,
            Enter::End => self.end(request).await,
            Enter::Sessions => self.sessions(request).await,
            Enter::Revoke(id) => self.revoke(request, id).await,
            Enter::Seeing(code) => self.seeing(request, code).await,
            Enter::Approving(code) => self.approving(request, code, body).await,
            Enter::Declining(code) => self.declining(request, code).await,
        }
    }

    /// The Fetch Metadata defence of the OWASP CSRF cheat sheet, for the routes that change state.
    fn origin(&self, request: &Request) -> Result<(), Step<D::Error>> {
        if self.origins.admits(request) { Ok(()) } else { Err(Step::Origin) }
    }

    fn json<T: Serialize>(status: Status, value: &T) -> Result<Reply, Step<D::Error>> {
        let body = serde_json::to_vec(value).map_err(Step::Json)?;
        Ok(Reply::new(status, Media::Json, body))
    }

    fn drawn(&self) -> Result<Drawn, Step<D::Error>> {
        Draws(&self.draw).drawn().map_err(|error| Step::Draw(Unnamed::new(Port::Draw, error)))
    }

    /// The session secret a request holds: its cookie. The API reads signatures and ignores
    /// cookies; this reads cookies and ignores signatures, so neither can be confused for the other.
    fn held(&self, request: &Request) -> Option<Proof> {
        request.cookie.as_deref().and_then(|header| self.config.jar.read(header))
    }

    async fn ask(&self, request: &Request, body: &[u8]) -> Result<Reply, Step<D::Error>> {
        let address = Unreadable::read::<Ask>(body).map_err(Step::Body)?.address.0;
        let now = self.clock.now();
        let who = Who { address: &address, source: request.source };
        self.limiter.admit(&self.config.rates, who, now).map_err(Step::Limited)?;
        let requested = self.entry.request(&address, self.drawn()?, now).await.map_err(Step::Sign)?;
        if let Requested::Issued(issued) = requested {
            let link = self.config.base.link(issued.proof.reveal());
            let valid = self.entry.settings().login.duration();
            // a full box is counted by the outbox and told by the courier; the answer is the same
            self.config.outbox.push(Message { to: address, template: Template::Login { link, valid } }).ok();
        }
        Self::json(Status::Accepted, &Asked { requested: true })
    }

    async fn look(&self, proof: Option<Proof>) -> Result<Reply, Step<D::Error>> {
        let proof = proof.ok_or_else(Step::unusable)?;
        let pending = self.entry.peek(&proof, self.clock.now()).await.map_err(Step::Sign)?;
        let pending = pending.ok_or_else(Step::unusable)?;
        Self::json(Status::Ok, &Looked { purpose: pending.purpose.name(), expires: pending.expires })
    }

    async fn redeem(&self, proof: Option<Proof>, body: &[u8]) -> Result<Reply, Step<D::Error>> {
        let proof = proof.ok_or_else(Step::unusable)?;
        let asked: Redeem =
            if body.is_empty() { Redeem::default() } else { Unreadable::read(body).map_err(Step::Body)? };
        let address = asked.address.map(|address| address.0);
        let now = self.clock.now();
        let entered = self
            .entry
            .redeem(Presented { proof: &proof, address: address.as_ref() }, self.drawn()?, now)
            .await
            .map_err(Step::Sign)?;
        let age = Duration::try_from(entered.session.limit.duration_since(now)).unwrap_or_default();
        let signed = Signed {
            session: entered.secret.reveal(),
            shown: Shown::from(&entered.session),
            created_account: entered.created,
        };
        let mut reply = Self::json(Status::Ok, &signed)?;
        reply.cookie = Some(self.config.jar.set(&entered.secret, age));
        Ok(reply)
    }

    async fn show(&self, request: &Request) -> Result<Reply, Step<D::Error>> {
        let proof = self.held(request).ok_or(Step::Anonymous)?;
        let session = self.entry.resolve(&proof, self.clock.now()).await.map_err(Step::Sign)?;
        Self::json(Status::Ok, &Shown::from(&session.ok_or(Step::Anonymous)?))
    }

    async fn end(&self, request: &Request) -> Result<Reply, Step<D::Error>> {
        let proof = self.held(request).ok_or(Step::Anonymous)?;
        if !self.entry.end(&proof, self.clock.now()).await.map_err(Step::Sign)? {
            return Err(Step::Anonymous);
        }
        let mut reply = Self::json(Status::Ok, &Ended { ended: true })?;
        reply.cookie = Some(self.config.jar.clear());
        Ok(reply)
    }
}

impl<D: Db, K: Clock, R: Draw> Door<D, K, R> {
    /// The live sessions of the account the request is signed in as, newest first, the asking one
    /// marked. No secret is in the list: an id names a session in a revoke and signs nothing.
    async fn sessions(&self, request: &Request) -> Result<Reply, Step<D::Error>> {
        let now = self.clock.now();
        let proof = self.held(request).ok_or(Step::Anonymous)?;
        let current = self.entry.resolve(&proof, now).await.map_err(Step::Sign)?.ok_or(Step::Anonymous)?;
        let live = self.entry.sessions(current.account, now).await.map_err(Step::Sign)?;
        let listed: Vec<Listed> =
            live.iter().map(|session| Listed { shown: Shown::from(session), current: session.id == current.id }).collect();
        Self::json(Status::Ok, &listed)
    }

    /// Ends one of the account's sessions. Another account's session, an ended one and an id that
    /// names nothing are one answer. Ending the asking session clears its cookie.
    async fn revoke(&self, request: &Request, id: Option<control::Id<control::SessionKind>>) -> Result<Reply, Step<D::Error>> {
        let now = self.clock.now();
        let proof = self.held(request).ok_or(Step::Anonymous)?;
        let current = self.entry.resolve(&proof, now).await.map_err(Step::Sign)?.ok_or(Step::Anonymous)?;
        let id = id.ok_or(Step::Missing)?;
        if !self.entry.revoke(current.account, id, now).await.map_err(Step::Sign)? {
            return Err(Step::Missing);
        }
        let mut reply = Self::json(Status::Ok, &Ended { ended: true })?;
        if id == current.id {
            reply.cookie = Some(self.config.jar.clear());
        }
        Ok(reply)
    }
}

impl<D: Db, K: Clock, R: Draw> Answers for Door<D, K, R> {
    fn answer(&self, request: &Request) -> impl std::future::Future<Output = Reply> {
        self.respond(request)
    }
}

/// What an approver is shown: the ask, as the requester made it.
#[derive(Debug, Serialize)]
struct Asking {
    fingerprint: String,
    label: String,
    kind: &'static str,
    site: Option<String>,
    rights: Vec<&'static str>,
    lifetime: String,
    asked: Timestamp,
}

/// A key approved, without anything secret.
#[derive(Debug, Serialize)]
struct Approved {
    fingerprint: String,
    expires: Timestamp,
}

/// An ask turned down.
#[derive(Debug, Serialize)]
struct Declined {
    declined: bool,
}

impl<D: Db, K: Clock, R: Draw> Approver for Door<D, K, R> {
    type Error = Error<D::Error>;

    /// The bootstrap adapter: the session of the dashboard cookie. A self-host has one person,
    /// the owner, who has every right.
    async fn approver(&self, request: &Request, now: Timestamp) -> Result<Option<Approving>, Self::Error> {
        let Some(proof) = self.held(request) else { return Ok(None) };
        let session = self.entry.resolve(&proof, now).await?;
        Ok(session.map(|session| Approving {
            account: session.account,
            rights: Rights::EVERY,
            strength: Strength::Session,
        }))
    }
}

impl<D: Db, K: Clock, R: Draw> Door<D, K, R> {
    /// The person a request is from, with the gate open: a wrong code is counted against it.
    async fn gated(&self, request: &Request) -> Result<(Approving, Timestamp), Step<D::Error>> {
        let now = self.clock.now();
        let who = self.approver(request, now).await.map_err(Step::Sign)?.ok_or(Step::Anonymous)?;
        self.enrol.gate().open(now).map_err(Step::Closed)?;
        Ok((who, now))
    }

    fn store(error: control::keys::Error<D::Error>) -> Step<D::Error> {
        match error.refusal() {
            Some(refused) => Step::Keys(refused.clone()),
            None => Step::Store(error),
        }
    }

    /// A code that names nothing is counted, and answered as every unusable code is.
    fn missed(&self, now: Timestamp) -> Step<D::Error> {
        self.enrol.gate().miss(now);
        Step::Keys(KeyRefused::NoEnrolment)
    }

    async fn seeing(&self, request: &Request, code: Option<control::keys::UserCode>) -> Result<Reply, Step<D::Error>> {
        let (_, now) = self.gated(request).await?;
        let Some(code) = code else { return Err(self.missed(now)) };
        let seen = self.enrol.keys().look(&code, now).await.map_err(Self::store)?;
        let seen = seen.ok_or_else(|| self.missed(now))?;
        let asking = Asking {
            fingerprint: seen.fingerprint.to_string(),
            label: seen.label.to_string(),
            kind: seen.purpose.name(),
            site: seen.site.map(|site| site.to_string()),
            rights: seen.rights.iter().map(control::keys::Right::name).collect(),
            lifetime: seen.lifetime.to_string(),
            asked: seen.asked,
        };
        Self::json(Status::Ok, &asking)
    }

    async fn approving(
        &self,
        request: &Request,
        code: Option<control::keys::UserCode>,
        body: &[u8],
    ) -> Result<Reply, Step<D::Error>> {
        let (who, now) = self.gated(request).await?;
        let asked: Approve = Unreadable::read(body).map_err(Step::Body)?;
        let Some(code) = code else { return Err(self.missed(now)) };
        let fingerprint = asked.fingerprint.0;
        let approval = Approval { code, fingerprint: fingerprint.clone(), by: who.account, within: who.rights };
        match self.enrol.keys().approve(approval, now).await {
            Ok(key) => Self::json(Status::Ok, &Approved { fingerprint: fingerprint.to_string(), expires: key.expires }),
            Err(error) => {
                if matches!(error.refusal(), Some(KeyRefused::NoEnrolment)) {
                    self.enrol.gate().miss(now);
                }
                Err(Self::store(error))
            }
        }
    }

    async fn declining(
        &self,
        request: &Request,
        code: Option<control::keys::UserCode>,
    ) -> Result<Reply, Step<D::Error>> {
        let (_, now) = self.gated(request).await?;
        let Some(code) = code else { return Err(self.missed(now)) };
        self.enrol.keys().deny(&code, now).await.map_err(Self::store)?;
        Self::json(Status::Ok, &Declined { declined: true })
    }
}

/// Two answerers, with the routes the second owns going to it: the API in front, the door
/// beside it (and a second `Routed` for the passkey routes beside that). One `Answers` for the server, and the API's own routes unchanged.
#[derive(Debug)]
pub struct Routed<A, B> {
    api: A,
    door: B,
}

impl<A, B> Routed<A, B> {
    /// `api` answers everything but the sign-in routes, which `door` answers.
    pub const fn new(api: A, door: B) -> Self {
        Self { api, door }
    }
}

/// An answerer that owns some paths: [`Routed`] sends it those, and everything else goes to the
/// answerer in front.
pub trait Owner: Answers {
    /// Whether `path` is one of its routes, whatever the method.
    fn owns(&self, path: &str) -> bool;
}

impl<D: Db, K: Clock, R: Draw> Owner for Door<D, K, R> {
    fn owns(&self, path: &str) -> bool {
        Enter::owns(path)
    }
}

impl<A: Answers, B: Owner> Answers for Routed<A, B> {
    async fn answer(&self, request: &Request) -> Reply {
        if self.door.owns(&request.path) { self.door.answer(request).await } else { self.api.answer(request).await }
    }
}
