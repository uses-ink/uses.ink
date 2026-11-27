//! `ink watch`: follow a build of the site to its end.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Args;

use super::follow::Follow;
use super::view::Pushed;
use crate::deploy::{Failure, Remote, Short};
use crate::error::Error;
use crate::exit::Exit;
use crate::output::Io;
use crate::stage::{StageLine, Verb};
use crate::term::Answer;

/// `ink watch [BUILD]`.
#[derive(Debug, Args)]
pub struct Watch {
    /// The build to follow, by the start of its id. Without one, the build that is running or
    /// queued, or else the next one the site starts.
    pub build: Option<String>,
    /// Seconds to wait before stopping; the build keeps going on the server.
    #[arg(long, value_name = "SECONDS", default_value_t = 300)]
    pub wait: u64,
    /// Milliseconds between two looks at the site.
    #[arg(long, value_name = "MS", default_value_t = 1000)]
    pub poll: u64,
    /// Which site, and how to reach it.
    #[command(flatten)]
    pub remote: Remote,
    /// The site's folder, from `-C`.
    #[arg(skip)]
    pub dir: PathBuf,
}

impl Watch {
    /// Follows the build, then says what it made.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let calls = self.remote.folder(&self.dir)?;
        let follow =
            Follow { calls: &calls, poll: Duration::from_millis(self.poll), wait: Duration::from_secs(self.wait) };
        let known = calls.block(calls.api.builds())?;
        let build = match &self.build {
            Some(written) => Follow::find(&known, written)?.id,
            None => match known.iter().rev().find(|build| !build.state.ended()) {
                Some(running) => running.id,
                None => self.next(&calls, &known, io)?,
            },
        };
        io.stage(&StageLine::new(Verb::Followed, format!("build {build}")))?;
        let ended = follow.until(build, io)?;
        let row = follow.settle(&ended)?;
        io.stage(&StageLine::new(Verb::Built, format!("deploy {} is live", Short(row.id))))?;
        io.result(&Answer::of(row.id))?;
        io.json(&Follow::document(Pushed::WATCH, calls.api.site.to_string(), &ended, Some(&row)))?;
        Ok(Exit::Done)
    }

    /// Waits for a build that `known` does not list.
    fn next(
        &self,
        calls: &crate::deploy::Calls,
        known: &[edge_api::view::BuildView],
        io: &mut Io<'_>,
    ) -> Result<build::BuildId, Error> {
        let started = Instant::now();
        io.stage(&StageLine::new(Verb::Followed, "the site, until it starts a build"))?;
        loop {
            let now = calls.block(calls.api.builds())?;
            if let Some(fresh) = now.iter().find(|build| known.iter().all(|old| old.id != build.id)) {
                return Ok(fresh.id);
            }
            if started.elapsed() >= Duration::from_secs(self.wait) {
                return Err(Failure::Waited { build: "next".to_owned(), after: Duration::from_secs(self.wait) }.into());
            }
            std::thread::sleep(Duration::from_millis(self.poll));
        }
    }
}
