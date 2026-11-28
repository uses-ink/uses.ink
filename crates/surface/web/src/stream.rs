//! A running build's log as a streamed page: chunks of one HTML document, the head first, a
//! line for each event as it arrives, and a last chunk with the result (decision 0053). The
//! edge sends each chunk as it comes; nothing here knows how.

use std::future::Future;

use build::Event;
use model::DeployId;
use unspliced::{Render, Split};

use crate::Cause;
use crate::Failure;
use crate::shell::Document;
use crate::view::build::{Ended, Line, Live, Outcome};

/// Where a running build's events come from: the builder's channel, or the stored log read as it
/// grows. The dashboard asks for the next one and waits.
pub trait Follow: Send {
    /// The next event, in order; `None` when the log ends.
    fn next(&mut self) -> impl Future<Output = Result<Option<Event>, Failure>> + Send;
}

/// Where a stream is.
#[derive(Debug)]
enum Phase {
    Head,
    Lines,
    Tail(Ended),
    Done,
}

/// A page sent in chunks.
#[derive(Debug)]
pub struct Streamed<F> {
    live: Live,
    follow: F,
    phase: Phase,
    deploy: Option<DeployId>,
}

impl<F: Follow> Streamed<F> {
    /// The stream of `live`'s page, its lines from `follow`.
    pub const fn new(live: Live, follow: F) -> Self {
        Self { live, follow, phase: Phase::Head, deploy: None }
    }

    /// The next chunk of the page, or `None` when it is whole. A chunk is a complete piece of
    /// markup: the head, one line, or the tail with the result.
    pub async fn chunk(&mut self) -> Option<Result<String, Failure>> {
        match std::mem::replace(&mut self.phase, Phase::Done) {
            Phase::Head => {
                self.phase = Phase::Lines;
                Some(self.head())
            }
            Phase::Lines => match self.follow.next().await {
                Ok(Some(event)) => {
                    self.phase = self.after(&event);
                    Some(Ok(Line(&event).html()))
                }
                Ok(None) => Some(self.tail(Ended::Cut(Failure::Cut { build: self.live.build }))),
                Err(failure) => Some(self.tail(Ended::Cut(failure))),
            },
            Phase::Tail(ended) => Some(self.tail(ended)),
            Phase::Done => None,
        }
    }

    /// The phase after `event`: the tail once the build says it finished.
    fn after(&mut self, event: &Event) -> Phase {
        match event {
            Event::Published { deploy, .. } => {
                self.deploy = Some(*deploy);
                Phase::Lines
            }
            Event::Finished { status, wall } => {
                Phase::Tail(Ended::Finished(Outcome { status: *status, deploy: self.deploy, wall: *wall }))
            }
            _ => Phase::Lines,
        }
    }

    fn head(&self) -> Result<String, Failure> {
        let page = Document(&self.live);
        let split = Split::new(&page).map_err(|marks| Failure::Render { source: Cause::of(&marks) })?;
        let mut text = String::new();
        split.head(&mut text).map_err(|halt| Failure::Render { source: Cause::of(&halt) })?;
        Ok(text)
    }

    fn tail(&self, ended: Ended) -> Result<String, Failure> {
        let live = self.live.ended(ended);
        let page = Document(&live);
        let split = Split::new(&page).map_err(|marks| Failure::Render { source: Cause::of(&marks) })?;
        let mut text = String::new();
        split.tail(&mut text).map_err(|halt| Failure::Render { source: Cause::of(&halt) })?;
        Ok(text)
    }
}
