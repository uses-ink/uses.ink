//! `farm status`: each host, its jobs, and where each declared job would go now.
use crate::cli::error::{Chain, Error};
use crate::cli::exit::Exit;
use crate::cli::version::{Freshness, Version};
use crate::config::jobs::Jobs;
use crate::host::Probed;
use crate::host::transport::Transport;
use crate::job::Job;
use crate::place::Placement;
use crate::run::Farm;
use crate::run::overview::Overview;
impl<T: Transport + Sync, P: Placement + Sync> Farm<T, P> {
    /// Every host: arch, load, slots, queue, jobs, the timings placement uses; and the old job
    /// state swept.
    pub(in crate::run) fn status(&self) -> Result<Exit, Error> {
        match Version::current().against(&self.tree.root) {
            Freshness::Current => {}
            other => {
                self.console.note(&other);
                if let Some(cause) = std::error::Error::source(&other) {
                    self.console.note(format!("caused by: {cause}"));
                }
            }
        }
        let probed = self.fleet.probe(&self.console);
        let jobs: Vec<Job> = Jobs::load_optional(&self.tree.root)?
            .map(|jobs| jobs.pick(&[]))
            .transpose()?
            .unwrap_or_default()
            .into_iter()
            .map(Job::from)
            .collect();
        let board = self.board(&probed);
        let overview = Overview::of(&probed, &board, &self.placement, &jobs);
        if self.flags.json {
            match serde_json::to_string_pretty(&overview) {
                Ok(text) => self.console.out(text),
                Err(error) => self.console.fail(format!("error: the status is not JSON: {error}")),
            }
        } else {
            probed.up.iter().for_each(|snap| self.console.out(snap));
            overview.hosts.iter().for_each(|row| self.console.out(row));
            overview.asleep.iter().for_each(|sleeping| self.console.out(sleeping));
            overview.next.iter().for_each(|next| self.console.out(next));
            self.usage();
            for (host, error) in &probed.down {
                self.console.out(format!("{host}: down\n  {}", Chain(error).to_string().replace('\n', "\n  ")));
            }
        }
        self.sweep(&probed);
        Ok(if probed.up.is_empty() { Exit::Farm } else { Exit::Passed })
    }

    /// Removes the state of old jobs on every host that is up, and says how many.
    pub(in crate::run) fn sweep(&self, probed: &Probed) {
        for snap in &probed.up {
            if let Some(member) = self.fleet.member(&snap.name) {
                match member.sweep() {
                    Ok(0) => {}
                    Ok(count) => {
                        self.console.note(format!("swept {count} job(s) older than seven days on `{}`", snap.name));
                    }
                    Err(error) => self.console.note(format!("sweeping `{}` failed: {}", snap.name, Chain(&error))),
                }
            }
        }
    }
}
