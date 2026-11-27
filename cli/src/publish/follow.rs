//! Following a build to its end: looking at the site's builds at an interval, saying each step,
//! and turning how it ended into a deploy or a failure.
//!
//! The server stores a build's log when the build ends, so what can be followed live is where the
//! build is (queued, running, ended), not which stage it is in; the log is read once, after.

use std::time::{Duration, Instant};

use super::view::Pushed;
use crate::deploy::wire::{Built, Row, Step};
use crate::deploy::{Calls, Failure, Logged, Short};
use crate::error::Error;
use crate::output::Io;
use crate::stage::{StageLine, Verb};
use crate::term::Phase;

/// How a build is followed: how often it is looked at, and for how long.
#[derive(Debug)]
pub(crate) struct Follow<'a> {
    pub(crate) calls: &'a Calls,
    /// The time between two looks.
    pub(crate) poll: Duration,
    /// The longest the build is waited for.
    pub(crate) wait: Duration,
}

impl Follow<'_> {
    /// How far along a step is, out of [`Follow::STEPS`].
    const fn done(step: Step) -> u64 {
        match step {
            Step::Queued => 0,
            Step::Running => 1,
            _ => 2,
        }
    }

    /// Steps of a build: queued, running, ended.
    const STEPS: u64 = 2;

    /// The build the site lists whose id starts with `written`, when it is one build.
    pub(crate) fn find(builds: &[Built], written: &str) -> Result<Built, Failure> {
        let mut found = builds.iter().filter(|build| build.id.starts_with(written));
        match (found.next(), found.next()) {
            (Some(build), None) => Ok(build.clone()),
            (Some(_), Some(_)) => Err(Failure::Ambiguous { written: written.to_owned(), count: 2 }),
            (None, _) => Err(Failure::Absent { build: written.to_owned() }),
        }
    }

    /// Waits for `build` to end, saying each step it goes through. A build that is not listed any
    /// more is [`Failure::Absent`], one that outlasts the wait is [`Failure::Waited`].
    pub(crate) fn until(&self, build: &str, io: &mut Io<'_>) -> Result<Built, Error> {
        let started = Instant::now();
        let mut seen: Option<Step> = None;
        io.begin(Phase::Building, Self::STEPS, "steps");
        loop {
            let builds = self.calls.block(self.calls.api.builds())?;
            let found = builds.iter().find(|listed| listed.id == build);
            let here = found.ok_or_else(|| Failure::Absent { build: build.to_owned() })?;
            if seen != Some(here.state) {
                io.end()?;
                io.stage(&StageLine::new(Verb::Followed, format!("build {build} is {}", here.state)))?;
                io.begin(Phase::Building, Self::STEPS, "steps");
                seen = Some(here.state);
            }
            io.tick(Self::done(here.state), Some(here.state.word()))?;
            if here.state.ended() {
                io.end()?;
                return Ok(here.clone());
            }
            if started.elapsed() >= self.wait {
                io.end()?;
                return Err(Failure::Waited { build: build.to_owned(), after: self.wait }.into());
            }
            std::thread::sleep(self.poll);
        }
    }

    /// What an ended build came to: the deploy it made, once the site lists it. A build that was
    /// refused or failed is its own failure, with the first problem its log names; a deploy that
    /// is recorded and not live because publishing is paused is [`Failure::Held`].
    pub(crate) fn settle(&self, built: &Built) -> Result<Row, Error> {
        match built.state {
            Step::Succeeded => {}
            state => {
                let log = self.calls.block(self.calls.api.log(&built.id))?.unwrap_or_default();
                let problems: Vec<String> = log.iter().filter_map(|line| Self::problem(line)).collect();
                let first = problems.first().cloned().map(Logged);
                return Err(Failure::Ended { build: built.id.clone(), state, count: problems.len(), first }.into());
            }
        }
        let deploys = self.calls.block(self.calls.api.deploys())?;
        let made = built.deploy.ok_or_else(|| Failure::Absent { build: built.id.clone() })?;
        let row = deploys
            .into_iter()
            .find(|row| row.id == made)
            .ok_or_else(|| Failure::Absent { build: format!("deploy {} of build {}", Short(made), built.id) })?;
        if row.live {
            return Ok(row);
        }
        // Not live is held back only while publishing is paused; otherwise a later deploy took over.
        let paused = self.calls.block(self.calls.api.overview())?.live.is_some_and(|live| live.paused);
        if paused { Err(Failure::Held { deploy: row.id }.into()) } else { Ok(row) }
    }

    /// The problem a line of a build's log names, written short, when it names one.
    fn problem(line: &str) -> Option<String> {
        let event: serde_json::Value = serde_json::from_str(line).ok()?;
        let kind = event.get("event")?.as_str()?;
        if !matches!(kind, "diagnosed" | "refused") {
            return None;
        }
        let mut shown = event.clone();
        shown.as_object_mut()?.remove("event");
        let text = shown.to_string();
        let cut: String = text.chars().take(240).collect();
        Some(format!("{kind}: {cut}"))
    }

    /// The document of a build that ended: `ink push` and `ink watch` print it.
    pub(crate) fn document(schema: &'static str, site: String, built: &Built, row: Option<&Row>) -> Pushed {
        Pushed {
            schema,
            site,
            tree: None,
            build: built.id.clone(),
            state: built.state,
            deploy: row.map(|row| row.id),
            live: row.map(|row| row.live),
            files: None,
            sent: None,
            held: None,
        }
    }
}
