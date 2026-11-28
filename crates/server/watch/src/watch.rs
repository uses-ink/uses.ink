//! The poll.

use logging::Field as _;
use std::collections::BTreeMap;
use std::error::Error;
use std::time::Duration;

use build::{Branch, BuildId, Pin, Revision, Source, Trigger};
use clock::{Clock, Draw, Slept};
use control::{Control, Id, NewJob, Ports, Requested, SiteKind};
use jiff::{SignedDuration, Timestamp};
use model::Oid;
use queue::Dedupe;

use crate::{Access, Branches, Head, Heads, Interval, Tip};

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
    /// The repository has more branches than the watch previews: this many were left out, the
    /// last by name.
    Capped {
        /// How many.
        left: usize,
    },
    /// A push that queues nothing, and why (decision 0161).
    Left(Unplaced),
}

/// Why a push to a site's repository queues no build.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Unplaced {
    /// The site has no git source: nothing a push could build.
    Source {
        /// The site.
        site: Id<SiteKind>,
    },
    /// The site names no branch and the event named no default, so the push cannot be told from
    /// a push to a branch that is only previewed.
    Default {
        /// The site.
        site: Id<SiteKind>,
        /// The branch pushed to.
        branch: Branch,
    },
    /// The push is to a branch the site does not build, and previews are off.
    Previews {
        /// The site.
        site: Id<SiteKind>,
        /// The branch pushed to.
        branch: Branch,
    },
}

impl std::fmt::Display for Unplaced {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source { site } => write!(out, "the site {site} has no git source, so a push builds nothing"),
            Self::Default { site, branch } => write!(
                out,
                "the site {site} follows the default branch and the push to `{branch}` did not say which it is",
                branch = branch.as_str()
            ),
            Self::Previews { site, branch } => {
                write!(
                    out,
                    "the push to `{}` is not the branch of the site {site} and previews are off",
                    branch.as_str()
                )
            }
        }
    }
}

impl std::error::Error for Unplaced {}

/// A push to a branch of a site's repository, as a forge told the watch of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pushed {
    /// The branch.
    pub branch: Branch,
    /// The commit it is at now.
    pub head: Head,
    /// The repository's default branch, when the event named one.
    pub default: Option<Branch>,
}

/// One site's poll.
#[derive(Debug)]
pub struct Polled {
    /// The site.
    pub site: Id<SiteKind>,
    /// What came of it.
    pub outcome: Outcome,
}

/// What the watch did about one branch of a site's repository (decision 0091).
#[derive(Debug)]
pub struct Branched {
    /// The site.
    pub site: Id<SiteKind>,
    /// The branch, or none when the branches could not be listed at all.
    pub branch: Option<Branch>,
    /// What came of it.
    pub outcome: Outcome,
}

/// How many branches of a repository the watch builds as previews: a setting, and `0` is none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Previews(pub usize);

impl Previews {
    /// The most branches of one site it builds when nothing says: ten.
    pub const DEFAULT: Self = Self(10);
}

/// One pass over the sites that were due.
#[derive(Debug, Default)]
pub struct Round {
    /// Each site that was polled.
    pub polled: Vec<Polled>,
    /// Each branch of a polled site that was queued or failed (decision 0091).
    pub branched: Vec<Branched>,
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
    previews: Previews,
    tracked: BTreeMap<Id<SiteKind>, Tracked>,
}

impl<'c, P: Ports + 'static, H: Heads, K: Clock, D: Draw> Watch<'c, P, H, K, D> {
    /// The shortest time the loop rests between rounds.
    pub const REST: Duration = Duration::from_secs(1);

    /// A watch that polls every `interval`. What it saw last is kept in the control plane, so a
    /// restart queues a build only for a source that moved while it was down.
    pub fn new(control: &'c Control<P>, heads: H, clocks: (K, D), interval: Interval) -> Self {
        Self {
            control,
            heads,
            clock: clocks.0,
            draw: clocks.1,
            interval,
            previews: Previews(0),
            tracked: BTreeMap::new(),
        }
    }

    /// The same watch, building up to `previews.0` branches of each public git site as previews.
    #[must_use]
    pub fn with_previews(self, previews: Previews) -> Self {
        Self { previews, ..self }
    }

    /// Polls every site that is due now, once.
    pub async fn step(&mut self) -> Round {
        let now = self.clock.now();
        let sites = match self.control.watched().await {
            Ok(sites) => sites,
            Err(error) => return Round { listing: Some(Box::new(error)), ..Round::default() },
        };
        self.tracked.retain(|id, _| sites.iter().any(|site| site.id == *id));
        let mut round = Round::default();
        for site in sites {
            if !matches!(site.source, Source::Git { .. }) {
                continue;
            }
            if !self.tracked.contains_key(&site.id) {
                match self.control.seen(site.id).await {
                    Ok(seen) => {
                        let seen = seen.and_then(|oid| oid.as_str().parse().ok());
                        let wait = self.interval.base();
                        self.tracked.insert(site.id, Tracked { seen, wait, due: now });
                    }
                    Err(error) => {
                        round.polled.push(Polled { site: site.id, outcome: Outcome::Failed(Box::new(error)) });
                        continue;
                    }
                }
            }
            let Some(tracked) = self.tracked.get(&site.id) else { continue };
            if tracked.due > now {
                continue;
            }
            let (head, open) = self.ask(&site).await;
            let outcome = self.settle(site.id, head, now).await;
            round.polled.push(Polled { site: site.id, outcome });
            if self.previews.0 > 0 && open {
                round.branched.extend(self.branches(&site, now).await);
            }
        }
        round
    }

    /// The head of a git site's branch, and whether the site is public (no credential).
    async fn ask(&self, site: &control::Site) -> (Result<Head, Box<dyn Error>>, bool) {
        let Source::Git { url, revision } = &site.source else { return (Err("not a git source".into()), false) };
        match Access::of(self.control, site.id).await {
            Ok(access) => {
                let asked = self.heads.head(url, revision.as_ref(), access.as_ref()).await;
                (asked.map_err(|error| Box::new(error) as Box<dyn Error>), access.is_none())
            }
            Err(error) => (Err(error), false),
        }
    }

    /// What a poll's answer means for the site: queues a build when the commit is new, and sets
    /// the next time to ask.
    async fn settle(&mut self, site: Id<SiteKind>, head: Result<Head, Box<dyn Error>>, now: Timestamp) -> Outcome {
        let interval = self.interval;
        let seen = self.tracked.get(&site).and_then(|tracked| tracked.seen.clone());
        let (outcome, moved) = match head {
            Err(error) => (Outcome::Failed(error), None),
            Ok(head) if seen.as_ref() == Some(&head) => (Outcome::Unchanged, None),
            // a push may have queued this commit since this loop last looked: the row says so
            Ok(head) => match self.control.seen(site).await {
                Ok(Some(stored)) if stored.as_str() == head.as_str() => (Outcome::Unchanged, Some(head)),
                Ok(_) => match self.queue(site, &head, (now, Trigger::Poll)).await {
                    Ok(outcome) => (outcome, Some(head)),
                    Err(outcome) => (outcome, None),
                },
                Err(error) => (Outcome::Failed(Box::new(error)), None),
            },
        };
        match &outcome {
            Outcome::Failed(error) => logging::warn!(
                name: logging::event::WATCH_POLLED,
                site = logging::Shown(site).field(),
                outcome = "failed",
                error = logging::Failure(error.as_ref()).field(),
            ),
            Outcome::Queued { build, .. } => logging::info!(
                name: logging::event::WATCH_QUEUED,
                site = logging::Shown(site).field(),
                build = logging::Shown(build).field(),
            ),
            other => logging::debug!(
                name: logging::event::WATCH_POLLED,
                site = logging::Shown(site).field(),
                outcome = if matches!(other, Outcome::Unchanged) { "unchanged" } else { "duplicate" },
            ),
        }
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

    /// Queues a preview build for each branch of a public site that has no build of its commit,
    /// at most [`Previews`] of them by name, leaving out the branch the site itself builds. A
    /// site with a credential has none: its branches are never served at an open address.
    async fn branches(&self, site: &control::Site, now: Timestamp) -> Vec<Branched> {
        let Source::Git { url, revision } = &site.source else { return Vec::new() };
        let said = |branch, outcome| Branched { site: site.id, branch, outcome };
        let listed = match self.heads.branches(url, None).await {
            Ok(listed) => listed,
            Err(error) => return vec![said(None, Outcome::Failed(Box::new(error)))],
        };
        let own = Self::own(&listed, revision.as_ref());
        let mut tips: Vec<&Tip> = listed.found.iter().filter(|tip| Some(&tip.branch) != own.as_ref()).collect();
        tips.sort_by(|left, right| left.branch.as_str().cmp(right.branch.as_str()));
        let left = tips.len().saturating_sub(self.previews.0);
        tips.truncate(self.previews.0);
        let mut done = Vec::new();
        for tip in tips {
            if let Some(outcome) = self.preview(site.id, tip, (now, Trigger::Poll)).await {
                done.push(said(Some(tip.branch.clone()), outcome));
            }
        }
        if left > 0 {
            done.push(said(None, Outcome::Capped { left }));
        }
        done
    }

    /// The branch the site's own build follows: the one its revision names, else the default.
    fn own(listed: &Branches, revision: Option<&Revision>) -> Option<Branch> {
        match revision {
            Some(named) => {
                listed.found.iter().map(|tip| &tip.branch).find(|branch| branch.as_str() == named.as_str()).cloned()
            }
            None => listed.default.clone(),
        }
    }
}

impl<'c, P: Ports + 'static, K: Clock, D: Draw> Watch<'c, P, (), K, D> {
    /// What a push needs and nothing of polling: a watch that only takes [`Watch::pushed`], over
    /// the control plane, a clock and a source of ids, building up to `previews.0` branches. It
    /// polls nothing, so it has no remote to ask.
    pub fn pusher(control: &'c Control<P>, clocks: (K, D), previews: Previews) -> Self {
        Self {
            control,
            heads: (),
            clock: clocks.0,
            draw: clocks.1,
            interval: Interval::default(),
            previews,
            tracked: BTreeMap::new(),
        }
    }
}

impl<P: Ports + 'static, H, K: Clock, D: Draw> Watch<'_, P, H, K, D> {
    /// Queues the preview build of one branch at its commit, unless one was queued before.
    async fn preview(&self, site: Id<SiteKind>, tip: &Tip, when: (Timestamp, Trigger)) -> Option<Outcome> {
        let (now, trigger) = when;
        let failed = |error: Box<dyn Error>| Some(Outcome::Failed(error));
        let revision: Revision = match tip.head.as_str().parse() {
            Ok(revision) => revision,
            Err(error) => return failed(Box::new(error)),
        };
        let pin = Pin { branch: tip.branch.clone(), revision };
        match self.control.pinned(site, &pin).await {
            Ok(true) => return None,
            Ok(false) => {}
            Err(error) => return failed(Box::new(error)),
        }
        let bytes = match self.draw.bytes() {
            Ok(bytes) => bytes,
            Err(error) => return failed(Box::new(error)),
        };
        let label = match control::Slug::branch(tip.branch.as_str()) {
            Ok(label) => label,
            Err(error) => return failed(Box::new(error)),
        };
        let key: String = format!("pv-{site}-{}-{label}", tip.head).chars().take(Dedupe::MAX).collect();
        let dedupe: Dedupe = match key.parse() {
            Ok(dedupe) => dedupe,
            Err(error) => return failed(Box::new(error)),
        };
        let new = NewJob { id: BuildId::from_bytes(bytes), site, trigger, dedupe: Some(dedupe), at: now };
        match self.control.request_branch(new, pin).await {
            Ok(Requested::Queued(build)) => Some(Outcome::Queued { build, head: tip.head.clone() }),
            Ok(_) => Some(Outcome::Duplicate { head: tip.head.clone() }),
            Err(error) => failed(Box::new(error)),
        }
    }

    /// Queues the build of a new commit.
    async fn queue(&self, site: Id<SiteKind>, head: &Head, when: (Timestamp, Trigger)) -> Result<Outcome, Outcome> {
        let (now, trigger) = when;
        let failed = |error: Box<dyn Error>| Outcome::Failed(error);
        let oid = Oid::parse(head.as_str()).map_err(|error| failed(Box::new(error)))?;
        let bytes = self.draw.bytes().map_err(|error| failed(Box::new(error)))?;
        let dedupe: Dedupe = format!("poll-{site}-{head}").parse().map_err(|error| failed(Box::new(error)))?;
        let new = NewJob { id: BuildId::from_bytes(bytes), site, trigger, dedupe: Some(dedupe), at: now };
        let requested = self.control.request_build(new).await.map_err(|error| failed(Box::new(error)))?;
        self.control.see(site, &oid, now).await.map_err(|error| failed(Box::new(error)))?;
        match requested {
            Requested::Queued(build) => Ok(Outcome::Queued { build, head: head.clone() }),
            _ => Ok(Outcome::Duplicate { head: head.clone() }),
        }
    }

    /// Queues what a push to a repository the site proved comes to, now and not at the next poll
    /// (decision 0161).
    ///
    /// A push to the branch the site builds queues a build of the commit with the trigger `push`
    /// and the dedupe key the poll uses, and records the commit as seen, so a push and the poll that
    /// follows it queue one build, even when the first is built before the poll looks. The branch
    /// is the one the site's revision names, else the repository's default branch as the event
    /// says it. A push to any other branch is a preview of it through the pin table, under the
    /// same cap and public-source rule as the poll's, and when previews are off it is
    /// [`Outcome::Left`], not a failure.
    ///
    /// This takes `&self`: it reads and writes only the control plane, so the API can answer a
    /// delivery with a watch of its own while the poll loop runs on another task.
    pub async fn pushed(&self, site: Id<SiteKind>, push: &Pushed) -> Outcome {
        let now = self.clock.now();
        let outcome = self.landed(site, push, now).await;
        match &outcome {
            Outcome::Failed(error) => logging::warn!(
                name: logging::event::WATCH_PUSHED,
                site = logging::Shown(site).field(),
                outcome = "failed",
                error = logging::Failure(error.as_ref()).field(),
            ),
            Outcome::Left(why) => logging::debug!(
                name: logging::event::WATCH_PUSHED,
                site = logging::Shown(site).field(),
                outcome = "left",
                error = logging::Failure(why).field(),
            ),
            Outcome::Queued { build, .. } => logging::info!(
                name: logging::event::WATCH_PUSHED,
                site = logging::Shown(site).field(),
                outcome = "queued",
                build = logging::Shown(build).field(),
            ),
            _ => logging::debug!(
                name: logging::event::WATCH_PUSHED,
                site = logging::Shown(site).field(),
                outcome = "duplicate",
            ),
        }
        outcome
    }

    /// What a push comes to, before it is told.
    async fn landed(&self, site: Id<SiteKind>, push: &Pushed, now: Timestamp) -> Outcome {
        let found = match self.control.site(site).await {
            Ok(found) => found,
            Err(error) => return Outcome::Failed(Box::new(error)),
        };
        let Source::Git { revision, .. } = &found.source else { return Outcome::Left(Unplaced::Source { site }) };
        let followed = match revision {
            Some(named) => named.as_str(),
            None => match &push.default {
                Some(default) => default.as_str(),
                None => return Outcome::Left(Unplaced::Default { site, branch: push.branch.clone() }),
            },
        };
        if followed == push.branch.as_str() {
            return match self.control.seen(site).await {
                Ok(Some(stored)) if stored.as_str() == push.head.as_str() => {
                    Outcome::Duplicate { head: push.head.clone() }
                }
                Ok(_) => self.queue(site, &push.head, (now, Trigger::Push)).await.unwrap_or_else(|outcome| outcome),
                Err(error) => Outcome::Failed(Box::new(error)),
            };
        }
        if self.previews.0 == 0 {
            return Outcome::Left(Unplaced::Previews { site, branch: push.branch.clone() });
        }
        let tip = Tip { branch: push.branch.clone(), head: push.head.clone() };
        self.preview(site, &tip, (now, Trigger::Push)).await.unwrap_or(Outcome::Duplicate { head: push.head.clone() })
    }
}

impl<P: Ports + 'static, H: Heads, K: Clock, D: Draw> Watch<'_, P, H, K, D> {
    /// How long to rest before the next round: until the first site is due, never less than
    /// [`Watch::REST`] and never more than the lookout, so a site added meanwhile is noticed.
    fn rest(&self) -> Duration {
        let now = self.clock.now();
        let soonest = self.tracked.values().map(|tracked| tracked.due).min();
        let until = soonest
            .map_or(self.interval.base(), |due| Duration::try_from(due.duration_since(now)).unwrap_or(Duration::ZERO));
        until.clamp(Self::REST, self.interval.lookout().max(Self::REST))
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
