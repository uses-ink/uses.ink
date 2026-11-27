//! `farm wait` and `farm log`: a detached job's end and its log.
use crate::cli::error::Error;
use crate::cli::exit::Exit;
use crate::cli::name::JobId;
use crate::host::transport::Transport;
use crate::job::event::Event;
use crate::job::summary::Summary;
use crate::job::{Finished, Served, Verdict};
use crate::place::{Need, Placement};
use crate::run::{Batch, Farm};
use std::time::Duration;

impl<T: Transport + Sync, P: Placement + Sync> Farm<T, P> {
    /// Blocks until a detached job is done, or the timeout passes.
    pub(in crate::run) fn wait(&self, id: &JobId) -> Result<Batch, Error> {
        let member = self.fleet.member(&id.host).ok_or_else(|| Error::NoHost { need: Need::Any, up: Vec::new() })?;
        let said = member.wait(id, self.flags.timeout)?;
        let mut batch = Batch::default();
        let finished = |verdict, summary, log| Finished {
            id: id.clone(),
            label: "wait".to_owned(),
            command: "started earlier; see `farm log`".to_owned(),
            host: id.host.clone(),
            slot: None,
            queued: Duration::ZERO,
            verdict,
            log,
            summary,
            served: Served::Ran,
        };
        let path = self.log_path(id);
        match said.events.first() {
            Some(Event::Done { code, ran }) => {
                let (path, text) = self.fetch_log(member, id)?;
                self.promote(id, (*code, *ran), (&path, &text));
                batch.done.push(finished(Verdict::Done { code: *code, ran: *ran }, Summary::of(&text), path));
            }
            Some(Event::Running) => batch.done.push(finished(Verdict::Running, Summary::default(), path)),
            Some(Event::NoJob) => return Err(Error::NoJob { job: id.clone() }),
            Some(Event::Lost) | None => return Err(Error::Lost { job: id.clone() }),
            Some(_) => return Err(Error::Silent { host: id.host.clone(), what: "how the job ended" }),
        }
        Ok(batch)
    }

    /// Prints a job's log from its host.
    pub(in crate::run) fn log(&self, id: &JobId) -> Result<Exit, Error> {
        let member = self.fleet.member(&id.host).ok_or_else(|| Error::NoHost { need: Need::Any, up: Vec::new() })?;
        let said = member.log(id, self.flags.tail)?;
        if said.events.contains(&Event::NoJob) {
            return Err(Error::NoJob { job: id.clone() });
        }
        said.output.iter().for_each(|line| self.console.out(line));
        Ok(Exit::Passed)
    }
}
