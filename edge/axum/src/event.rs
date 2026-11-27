//! What the server tells the operator, as data.

use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

use axum::http::StatusCode;
use edge::{Chain, Hostname, Outcome, Refusal, Stage, Status};
use trace::Field as _;

use crate::fault::{AnswerFault, ConnectionFault};
use crate::respond::Code;

/// How an answered request ended, without the data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Kind {
    /// A route served.
    Served,
    /// `304`.
    Unchanged,
    /// `301`.
    Moved,
    /// `404`.
    Missing,
    /// The health probe.
    Probed,
    /// Refused as hostile or over a limit.
    Refused,
    /// Failed on our side.
    Failed,
}

impl<F> From<&Outcome<F>> for Kind {
    fn from(outcome: &Outcome<F>) -> Self {
        match outcome {
            Outcome::Served { .. } => Self::Served,
            Outcome::Unchanged { .. } => Self::Unchanged,
            Outcome::Moved { .. } => Self::Moved,
            Outcome::Missing { .. } => Self::Missing,
            Outcome::Probed => Self::Probed,
            Outcome::Refused(_) => Self::Refused,
            _ => Self::Failed,
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Served => "served",
            Self::Unchanged => "unchanged",
            Self::Moved => "moved",
            Self::Missing => "missing",
            Self::Probed => "probed",
            Self::Refused => "refused",
            Self::Failed => "failed",
        })
    }
}

/// One request answered. It holds the target without its query, which can hold a person's words,
/// and never a header, so nothing here is a credential.
#[derive(Debug)]
pub struct Answered {
    /// The method, as sent.
    pub method: String,
    /// The target up to its `?`, cut at 200 characters.
    pub path: String,
    /// The host, when the request named a valid one.
    pub hostname: Option<Hostname>,
    /// The status sent.
    pub status: Status,
    /// How it ended.
    pub kind: Kind,
    /// Time from the request to its answer's head.
    pub took: Duration,
    /// Why, when it was refused.
    pub refusal: Option<Refusal>,
    /// What failed, when it failed.
    pub fault: Option<AnswerFault>,
}

/// One API call answered. It holds the path (site ids, never a token), the status and how long
/// it took; the token and the body are never in an event.
#[derive(Debug)]
pub struct Called {
    /// The method, as sent.
    pub method: String,
    /// The path, cut at 200 bytes.
    pub path: String,
    /// The status sent.
    pub status: u16,
    /// Time from the request to its reply.
    pub took: Duration,
}

/// A response that stopped partway: its reader stopped taking it or it ran past its time.
#[derive(Debug)]
pub struct Cut {
    /// Which wait ran out.
    pub stage: Stage,
    /// The budget.
    pub limit: Duration,
    /// Bytes already sent.
    pub sent: u64,
}

/// How a stop ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Stopped {
    /// Every connection finished within the drain time.
    Drained,
    /// The drain time ran out with connections still open; they were dropped.
    Abandoned {
        /// The drain budget.
        limit: Duration,
    },
}

/// Something the server did or met.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    /// Accepting connections.
    Listening {
        /// Where.
        address: SocketAddr,
    },
    /// A request answered.
    Answered(Answered),
    /// An API call answered.
    Called(Called),
    /// A response cut short.
    Cut(Cut),
    /// A connection or an accept failed; the server goes on.
    Connection {
        /// What failed.
        source: ConnectionFault,
    },
    /// Told to stop; no new connections.
    Stopping,
    /// Stopped.
    Stopped(Stopped),
}

impl fmt::Display for Answered {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let host = self.hostname.as_ref().map_or("-", |hostname| hostname.as_str());
        write!(out, "{} {host} {:?} {} {} {:?}", self.method, self.path, self.status, self.kind, self.took)?;
        if let Some(refusal) = &self.refusal {
            write!(out, ": {}", Chain(refusal))?;
        }
        if let Some(fault) = &self.fault {
            write!(out, ": {}", Chain(fault))?;
        }
        Ok(())
    }
}

impl fmt::Display for Event {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Listening { address } => write!(out, "listening on {address}"),
            Self::Answered(answered) => write!(out, "{answered}"),
            Self::Called(Called { method, path, status, took }) => {
                write!(out, "api {method} {path:?} {status} {took:?}")
            }
            Self::Cut(Cut { stage, limit, sent }) => {
                write!(out, "response cut after {sent} bytes: {stage} took more than {limit:?}")
            }
            Self::Connection { source } => write!(out, "connection: {}", Chain(source)),
            Self::Stopping => write!(out, "stopping"),
            Self::Stopped(Stopped::Drained) => write!(out, "stopped"),
            Self::Stopped(Stopped::Abandoned { limit }) => write!(out, "stopped with connections left after {limit:?}"),
        }
    }
}

/// Where events go. Called from many tasks at once, so it takes `&self`.
pub trait Observe: Send + Sync + 'static {
    /// One event.
    fn event(&self, event: &Event);
}

/// A request path as the log shows it: its first [`Clip::LIMIT`] characters. The query is never
/// part of what is passed in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Clip<'a>(pub(crate) &'a str);

impl Clip<'_> {
    /// How many characters of a path are logged.
    pub(crate) const LIMIT: usize = 200;
}

impl fmt::Display for Clip<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.chars().take(Self::LIMIT).try_for_each(|c| fmt::Write::write_char(out, c))
    }
}

impl Event {
    /// Writes the event to the log as a typed `trace` event: every field a number, a flag or a
    /// short name, and an error with its whole chain. Never the query of a request, never a
    /// header.
    pub(crate) fn emit(&self) {
        match self {
            Self::Listening { address } => {
                trace::info!(name: trace::event::EDGE_LISTENING, address = trace::Shown(address).field());
            }
            Self::Answered(answered) => answered.emit(),
            Self::Called(Called { method, path, status, took }) => trace::info!(
                name: trace::event::API_ANSWERED,
                method = method.as_str(),
                path = trace::Shown(path).field(),
                status = trace::Count(u64::from(*status)).field(),
                took_us = trace::Took(*took).field(),
            ),
            Self::Cut(Cut { stage, limit, sent }) => trace::warn!(
                name: trace::event::RESPONSE_CUT,
                stage = trace::Shown(stage).field(),
                limit_us = trace::Took(*limit).field(),
                sent = trace::Size(*sent).field(),
            ),
            Self::Connection { source } => {
                trace::error!(name: trace::event::CONNECTION_FAILED, error = trace::Failure(source).field());
            }
            Self::Stopping => trace::info!(name: trace::event::EDGE_STOPPING, accepting = false),
            Self::Stopped(Stopped::Drained) => trace::info!(name: trace::event::EDGE_STOPPED, drained = true),
            Self::Stopped(Stopped::Abandoned { limit }) => trace::warn!(
                name: trace::event::EDGE_STOPPED,
                drained = false,
                limit_us = trace::Took(*limit).field(),
            ),
        }
    }
}

impl Answered {
    fn emit(&self) {
        let status = trace::Count(u64::from(StatusCode::from(Code(self.status)).as_u16())).field();
        let host = self.hostname.as_ref().map(|hostname| trace::Shown(hostname.as_str()).field());
        let took = trace::Took(self.took).field();
        match (&self.refusal, &self.fault, self.kind) {
            (Some(refusal), _, _) => trace::warn!(
                name: trace::event::REQUEST_REFUSED,
                method = self.method.as_str(),
                path = trace::Shown(&self.path).field(),
                host,
                status,
                took_us = took,
                error = trace::Failure(refusal).field(),
            ),
            (_, Some(fault), _) => trace::error!(
                name: trace::event::REQUEST_FAILED,
                method = self.method.as_str(),
                path = trace::Shown(&self.path).field(),
                host,
                status,
                took_us = took,
                error = trace::Failure(fault).field(),
            ),
            (_, _, kind) => trace::info!(
                name: trace::event::REQUEST_ANSWERED,
                method = self.method.as_str(),
                path = trace::Shown(&self.path).field(),
                host,
                status,
                outcome = trace::Kind::kind(kind),
                took_us = took,
            ),
        }
    }
}

impl trace::Kind for Kind {
    fn kind(self) -> &'static str {
        match self {
            Self::Served => "served",
            Self::Unchanged => "unchanged",
            Self::Moved => "moved",
            Self::Missing => "missing",
            Self::Probed => "probed",
            Self::Refused => "refused",
            Self::Failed => "failed",
        }
    }
}

/// An event with its log line: what the server and the response body call instead of
/// `Observe::event`, so one call writes both.
pub(crate) trait Logged: Observe {
    /// Logs the event, then hands it to the observer.
    fn say(&self, event: Event) {
        event.emit();
        self.event(&event);
    }
}

impl<O: Observe + ?Sized> Logged for O {}
