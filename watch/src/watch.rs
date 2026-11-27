//! The poll.

use std::collections::BTreeMap;
use std::error::Error;
use std::time::Duration;

use build::{BuildId, Source, Trigger};
use clock::{Clock, Draw, Slept};
use control::{Control, Id, NewJob, Ports, Requested, SiteKind};
use jiff::{SignedDuration, Timestamp};
use queue::Dedupe;

use crate::{Head, Heads, Interval};

/// What polling one site came to.
#[derive(Debug)]
#[non_exhaustive]
pub enum Outcome {
    /// The source moved and a build is queued.
    Queued {
        /// The build.
        build: BuildId,
        /// The commit.
        head: Head,
    },
    /// The source moved, and a build of this commit is queued already.
    Duplicate {
        /// The commit.
        head: Head,
    },
    /// The source is where it was.
    Unchanged,
    /// The remote did not say, or the build could not be queued. The site is asked again after
    /// a longer wait.
    Failed(Box<dyn Error>),
}

/// One site's poll.
#[derive(Debug)]
pub struct Polled {
    /// The site.
    pub site: Id<SiteKind>,
    /// What came of it.
    pub outcome: Outcome,
}

/// One pass over the sites that were due.
#[derive(Debug, Default)]
pub struct Round {
    /// Each site that was polled.
    pub polled: Vec<Polled>,
    /// Why the sites could not be listed, when that failed.
    pub listing: Option<Box<dyn Error>>,
}

/// Where a loop tells what it did.
pub trait Report<T> {
    /// One thing happened.
    fn report(&self, item: &T);
}

/// What the watch remembers of one site.
#[derive(Debug)]
struct Tracked {
    seen: Option<Head>,
    wait: Duration,
    due: Timestamp,
}

/// The polling watch over a control plane, a remote, a clock and a source of ids.
#[derive(Debug)]
pub struct Watch<'c, P: Ports, H, K, D> {
    control: &'c Control<P>,
    heads: H,
    clock: K,
    draw: D,
    interval: Interval,
    tracked: BTreeMap<Id<SiteKind>, Tracked>,
}

impl<'c, P: Ports + 'static, H: Heads, K: Clock, D: Draw> Watch<'c, P, H, K, D> {
    /// The shortest time the loop rests between rounds.
    pub const REST: Duration = Duration::from_secs(1);

    /// A watch that polls every `interval`. It remembers nothing across restarts: the first poll
    /// of a site after one queues a build, which is the same deploy when nothing changed.
    pub fn new(control: &'c Control<P>, heads: H, clocks: (K, D), interval: Interval) -> Self {
        Self { control, heads, clock: clocks.0, draw: clocks.1, interval, tracked: BTreeMap::new() }
    }

    /// Polls every site that is due now, once.
    pub async fn step(&mut self) -> Round {
        let now = self.clock.now();
        let sites = match self.control.watched().await {
            Ok(sites) => sites,
            Err(error) => return Round { polled: Vec::new(), listing: Some(Box::new(error)) },
        };
        self.tracked.retain(|id, _| sites.iter().any(|site| site.id == *id));
        let mut round = Round::default();
        for site in sites {
            let Source::Git { url, revision } = &site.source else { continue };
            let base = self.interval.base();
            let tracked = self.tracked.entry(site.id).or_insert(Tracked { seen: None, wait: base, due: now });
            if tracked.due > now {
                continue;
            }
            let head = self.heads.head(url, revision.as_ref()).await;
            let outcome = self.settle(site.id, head, now).await;
            round.polled.push(Polled { site: site.id, outcome });
        }
        round
    }

    /// What a poll's answer means for the site: queues a build when the commit is new, and sets
    /// the next time to ask.
    async fn settle(&mut self, site: Id<SiteKind>, head: Result<Head, H::Error>, now: Timestamp) -> Outcome {
        let interval = self.interval;
        let seen = self.tracked.get(&site).and_then(|tracked| tracked.seen.clone());
        let (outcome, moved) = match head {
            Err(error) => (Outcome::Failed(Box::new(error)), None),
            Ok(head) if seen.as_ref() == Some(&head) => (Outcome::Unchanged, None),
            Ok(head) => match self.queue(site, &head, now).await {
                Ok(outcome) => (outcome, Some(head)),
                Err(outcome) => (outcome, None),
            },
        };
        if let Some(tracked) = self.tracked.get_mut(&site) {
            tracked.wait = if moved.is_some() { interval.base() } else { interval.backed(tracked.wait) };
            if moved.is_some() {
                tracked.seen = moved;
            }
            tracked.due =
                now.checked_add(SignedDuration::try_from(tracked.wait).unwrap_or(SignedDuration::MAX)).unwrap_or(now);
        }
        outcome
    }

    /// Queues the build of a new commit.
    async fn queue(&self, site: Id<SiteKind>, head: &Head, now: Timestamp) -> Result<Outcome, Outcome> {
        let failed = |error: Box<dyn Error>| Outcome::Failed(error);
        let bytes = self.draw.bytes().map_err(|error| failed(Box::new(error)))?;
        let dedupe: Dedupe = format!("poll-{site}-{head}").parse().map_err(|error| failed(Box::new(error)))?;
        let new =
            NewJob { id: BuildId::from_bytes(bytes), site, trigger: Trigger::Poll, dedupe: Some(dedupe), at: now };
        match self.control.request_build(new).await {
            Ok(Requested::Queued(build)) => Ok(Outcome::Queued { build, head: head.clone() }),
            Ok(_) => Ok(Outcome::Duplicate { head: head.clone() }),
            Err(error) => Err(failed(Box::new(error))),
        }
    }

    /// How long to rest before the next round: until the first site is due, never less than
    /// [`Watch::REST`] and never more than the longest wait.
    fn rest(&self) -> Duration {
        let now = self.clock.now();
        let soonest = self.tracked.values().map(|tracked| tracked.due).min();
        let until = soonest
            .map_or(self.interval.base(), |due| Duration::try_from(due.duration_since(now)).unwrap_or(Duration::ZERO));
        until.clamp(Self::REST, self.interval.max())
    }

    /// Polls, tells `report`, rests, and does it again, until the clock says it is stopped. The
    /// system clock never does: the caller drops the future to stop it.
    pub async fn run(&mut self, report: &impl Report<Round>) {
        loop {
            let round = self.step().await;
            report.report(&round);
            if self.clock.sleep(self.rest()).await == Slept::Stopped {
                return;
            }
        }
    }
}
