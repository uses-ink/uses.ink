//! A blob as a response body: a chunk at a time, under an idle and a whole-response deadline.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use bytes::Bytes;
use edge::{Chunks, Stage};
use futures_util::stream;
use tokio::sync::OwnedSemaphorePermit;
use tokio::time::Instant;

use crate::event::{Cut, Event, Logged as _, Observe};

/// Why a body stopped: the connection is closed, since the status is already sent.
#[derive(Debug)]
pub(crate) enum Stopped<E> {
    /// A wait ran out.
    Slow { stage: Stage, limit: Duration },
    /// The blob failed to read.
    Read(E),
}

impl<E> fmt::Display for Stopped<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Slow { stage, limit } => write!(out, "{stage} took more than {limit:?}"),
            Self::Read(_) => write!(out, "a blob failed to read while it was sent"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Stopped<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Slow { .. } => None,
            Self::Read(source) => Some(source),
        }
    }
}

/// What a body is read under.
#[derive(Debug)]
pub(crate) struct Pump<C, O> {
    pub(crate) chunks: C,
    pub(crate) idle: Duration,
    pub(crate) send: Duration,
    pub(crate) deadline: Instant,
    pub(crate) sent: u64,
    pub(crate) observe: Arc<O>,
    /// Held until the body ends, so a request in flight counts until its last byte.
    #[expect(dead_code, reason = "never read: dropping it with the body is its whole job")]
    pub(crate) permit: Option<OwnedSemaphorePermit>,
}

impl<C: Chunks + 'static, O: Observe> Pump<C, O> {
    /// The stream of the body's chunks.
    pub(crate) fn body(self) -> Body {
        Body::from_stream(stream::unfold(Some(self), |state| async move {
            let mut pump = state?;
            let wait = pump.idle.min(pump.deadline.saturating_duration_since(Instant::now()));
            match tokio::time::timeout(wait, pump.chunks.next()).await {
                Ok(Ok(Some(chunk))) => {
                    pump.sent += chunk.len() as u64;
                    Some((Ok::<Bytes, Stopped<C::Error>>(chunk), Some(pump)))
                }
                Ok(Ok(None)) => None,
                Ok(Err(source)) => Some((Err(Stopped::Read(source)), None)),
                Err(_) => {
                    let (stage, limit) =
                        if wait == pump.idle { (Stage::Idle, pump.idle) } else { (Stage::Send, pump.send) };
                    pump.observe.say(Event::Cut(Cut { stage, limit, sent: pump.sent }));
                    Some((Err(Stopped::Slow { stage, limit }), None))
                }
            }
        }))
    }
}
