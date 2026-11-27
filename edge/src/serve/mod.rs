//! The edge: one request in, one answer out.

use std::time::Duration;

use model::{Chain, HeaderName, Route};

use crate::http::answer::{Answer, Fault, Outcome};
use crate::http::fields::{Fields, Hardening};
use crate::http::limits::Limits;
use crate::http::refusal::Refusal;
use crate::http::request::{Asked, Request, Scheme};
use crate::http::response::{Body, Name, Response, Status};
use crate::serve::error::Error;
use crate::serve::hostname::Hostname;
use crate::serve::port::{Blobs, Resolved, Sites};

pub(crate) mod error;
pub(crate) mod hostname;
pub(crate) mod port;
pub(crate) mod target;

/// How long browsers are told to remember that HTTPS is the only way to this host name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hsts {
    /// `max-age`.
    pub max_age: Duration,
}

impl Default for Hsts {
    /// A year.
    fn default() -> Self {
        Self { max_age: Duration::from_hours(365 * 24) }
    }
}

/// Serves sites: a host and a path in, a typed response out. It reads through [`Sites`] and
/// [`Blobs`] and touches no socket, file or clock, so every branch is testable with in-memory ports.
#[derive(Debug)]
pub struct Edge<S, B> {
    pub(crate) sites: S,
    pub(crate) blobs: B,
    pub(crate) limits: Limits,
    pub(crate) hardening: Hardening,
    pub(crate) page: Route,
    probe: Option<Route>,
}

/// The reply an edge over `S` and `B` gives.
pub type Reply<S, B> = Answer<<B as Blobs>::Blob, Fault<<S as Sites>::Error, <B as Blobs>::Error>>;

/// Who a request is for and how it came: what every response is built against.
#[derive(Clone, Debug)]
pub struct Caller {
    /// The host the request named, when it got far enough to name one.
    pub hostname: Option<Hostname>,
    /// How the client reached the edge.
    pub scheme: Scheme,
}

impl<S: Sites, B: Blobs<Scope = S::Scope>> Edge<S, B> {
    /// An edge serving what `sites` resolves, reading blobs from `blobs`, within `limits`. It
    /// sends no HSTS and answers no probe until told to.
    pub fn new(sites: S, blobs: B, limits: Limits) -> Result<Self, Error> {
        Ok(Self { sites, blobs, limits, hardening: Hardening::new(None)?, page: "/404".parse()?, probe: None })
    }

    /// Sends `Strict-Transport-Security` on every response to a request that came over TLS.
    pub fn with_hsts(mut self, hsts: Hsts) -> Result<Self, Error> {
        let value = model::HeaderValue::parse(format!("max-age={}", hsts.max_age.as_secs()))
            .map_err(|source| crate::FieldError { name: Name::Entry(HeaderName::StrictTransportSecurity), source })?;
        self.hardening = Hardening::new(Some(&value))?;
        Ok(self)
    }

    /// Answers `200 ok` at `route` on a host no site has, so a load balancer can probe by address
    /// without a name. A site's own page at that route is served as usual.
    #[must_use]
    pub fn with_probe(mut self, route: Route) -> Self {
        self.probe = Some(route);
        self
    }

    /// The budgets the edge enforces.
    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    /// Answers `request`. Never panics and never fails: every request, hostile or broken, gets a
    /// response and an [`Outcome`] saying why.
    pub async fn respond(&self, request: &Request<'_>) -> Reply<S, B> {
        match request.ask(&self.limits) {
            Ok(asked) => self.serve(&asked).await,
            Err(refusal) => self.refuse(refusal, &Caller { hostname: None, scheme: request.scheme }),
        }
    }

    async fn serve(&self, asked: &Asked) -> Reply<S, B> {
        let caller = Caller { hostname: Some(asked.hostname.clone()), scheme: asked.scheme };
        let route = asked.target.route().ok();
        let resolved = match self.sites.resolve(&asked.hostname).await {
            Ok(Some(resolved)) => resolved,
            Ok(None) if route.is_some() && route == self.probe => return self.probed(&caller),
            Ok(None) => return self.refuse(Refusal::Unserved { hostname: asked.hostname.clone() }, &caller),
            Err(source) => return self.fail(Fault::Sites(source), &caller),
        };
        self.found(&Call { asked, caller, resolved: &resolved, route }).await
    }

    /// The `200 ok` of a probe.
    fn probed(&self, caller: &Caller) -> Reply<S, B> {
        let mut fields = Fields::default();
        fields.set(Name::ContentType, model::MediaType::Text);
        fields.set(Name::ContentLength, 2);
        fields.set(Name::Entry(HeaderName::CacheControl), "no-store");
        self.finish(caller, Made::new(Status::Ok, fields, Body::Text("ok".to_owned()), Outcome::Probed))
    }

    /// The answer for a refusal: its status, its reason as text, and no caching. The adapter
    /// uses it for the refusals only it can make (too busy, too slow).
    pub fn refuse(&self, refusal: Refusal, caller: &Caller) -> Reply<S, B> {
        let text = Chain::new(&refusal).to_string();
        let mut fields = Fields::default();
        fields.set(Name::ContentType, model::MediaType::Text);
        fields.set(Name::ContentLength, text.len());
        fields.set(Name::Entry(HeaderName::CacheControl), "no-store");
        match &refusal {
            Refusal::Method { .. } => fields.set(Name::Allow, crate::http::request::Method::ALLOWED),
            Refusal::Unsatisfiable { length } => fields.set(Name::ContentRange, format_args!("bytes */{length}")),
            Refusal::Busy { .. } => fields.set(Name::RetryAfter, 1),
            _ => {}
        }
        let status = refusal.status();
        self.finish(caller, Made::new(status, fields, Body::Text(text), Outcome::Refused(refusal)))
    }

    /// The answer for a failure of ours: `500`, the cause kept for the log and not shown.
    pub(crate) fn fail(&self, fault: Fault<S::Error, B::Error>, caller: &Caller) -> Reply<S, B> {
        let text = "the edge could not serve this request";
        let mut fields = Fields::default();
        fields.set(Name::ContentType, model::MediaType::Text);
        fields.set(Name::ContentLength, text.len());
        fields.set(Name::Entry(HeaderName::CacheControl), "no-store");
        self.finish(caller, Made::new(Status::Internal, fields, Body::Text(text.to_owned()), Outcome::Failed(fault)))
    }

    /// Adds what every response carries and checks every header value once.
    pub(crate) fn finish(&self, caller: &Caller, made: Made<B::Blob, Fault<S::Error, B::Error>>) -> Reply<S, B> {
        let Made { status, mut fields, body, outcome } = made;
        self.hardening.apply(&mut fields, caller.scheme);
        match fields.finish() {
            Ok(fields) => {
                Answer { response: Response { status, fields, body }, outcome, hostname: caller.hostname.clone() }
            }
            Err(error) => Answer {
                response: Response {
                    status: Status::Internal,
                    fields: self.hardening.all(caller.scheme),
                    body: Body::Empty,
                },
                outcome: Outcome::Failed(Fault::Field(error)),
                hostname: caller.hostname.clone(),
            },
        }
    }
}

/// A request that found its site: what serving a route reads.
#[derive(Debug)]
pub(crate) struct Call<'a, Scope> {
    pub(crate) asked: &'a Asked,
    pub(crate) caller: Caller,
    pub(crate) resolved: &'a Resolved<Scope>,
    pub(crate) route: Option<Route>,
}

/// A response before every response's headers are added.
#[derive(Debug)]
pub(crate) struct Made<R: crate::serve::port::Blob, F> {
    pub(crate) status: Status,
    pub(crate) fields: Fields,
    pub(crate) body: Body<R>,
    pub(crate) outcome: Outcome<F>,
}

impl<R: crate::serve::port::Blob, F> Made<R, F> {
    pub(crate) fn new(status: Status, fields: Fields, body: Body<R>, outcome: Outcome<F>) -> Self {
        Self { status, fields, body, outcome }
    }
}
