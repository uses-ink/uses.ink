//! What the server tells the operator, as data.

use std::error::Error;
use std::fmt;
use std::net::SocketAddr;
use std::time::Duration;

use edge::{Chain, Hostname, Outcome, Refusal, Stage, Status};

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
    /// The target up to its `?`, cut at 200 bytes.
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
    pub fault: Option<Box<dyn Error + Send + Sync>>,
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
    /// A response cut short.
    Cut(Cut),
    /// A connection or an accept failed; the server goes on.
    Connection {
        /// What failed.
        source: Box<dyn Error + Send + Sync>,
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
            write!(out, ": {}", Chain(fault.as_ref()))?;
        }
        Ok(())
    }
}

impl fmt::Display for Event {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Listening { address } => write!(out, "listening on {address}"),
            Self::Answered(answered) => write!(out, "{answered}"),
            Self::Cut(Cut { stage, limit, sent }) => {
                write!(out, "response cut after {sent} bytes: {stage} took more than {limit:?}")
            }
            Self::Connection { source } => write!(out, "connection: {}", Chain(source.as_ref())),
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

/// Events to `tracing`: refusals at warn, failures at error, the rest at info.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tracing;

impl Observe for Tracing {
    fn event(&self, event: &Event) {
        match event {
            Event::Answered(Answered { kind: Kind::Failed, .. }) | Event::Connection { .. } => {
                tracing::error!(target: "edge", %event)
            }
            Event::Answered(Answered { kind: Kind::Refused, .. }) | Event::Cut(_) => {
                tracing::warn!(target: "edge", %event)
            }
            _ => tracing::info!(target: "edge", %event),
        }
    }
}
