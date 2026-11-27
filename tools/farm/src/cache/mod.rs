//! What the farm remembers on this machine so a job is not run twice for nothing, kept in one
//! directory per repo (`$XDG_CACHE_HOME/uses-ink/farm/<repo>`, else `~/.cache/...`), shared by
//! every workspace of the repo:
//!
//! - [`Store`]: the passes. A job that passed is keyed by [`Key`] (its name, command, input
//!   digest, host class, toolchain and job file), so a rebase onto a tree with the same inputs
//!   finds it. Failures are never stored.
//! - [`Marks`]: the jobs running now, so an identical job attaches to the one running.
//! - [`Journal`]: what each workspace ran, for `farm status` and the budget.

mod claim;
mod fault;
pub(crate) mod hash;
pub(crate) mod inputs;
mod journal;
mod moment;
mod store;

use std::path::Path;

use crate::cache::inputs::Inputs;
use crate::config::jobs::Limits;
use crate::place::history::{History, Memory};
use crate::place::wake::Seen;

pub(crate) use claim::{Claim, Claimed, Mark, Marks};
pub(crate) use fault::{Causes, Fault, Trouble};
pub(crate) use hash::key::{Facts, Key};
pub(crate) use journal::{Journal, Kind, Record};
pub(crate) use moment::{Ago, Moment};
pub(crate) use store::{Entry, Flaky, Store};

/// The cache of one run: the passes, the running jobs, the journal and the digest maker.
#[derive(Debug)]
pub(crate) struct Cache {
    pub(crate) store: Store,
    pub(crate) marks: Marks,
    pub(crate) journal: Journal,
    pub(crate) history: History,
    pub(crate) seen: Seen,
    pub(crate) inputs: Inputs,
}

impl Cache {
    pub(crate) fn at(dir: &Path, limits: Limits, memory: Memory, inputs: Inputs) -> Self {
        Self {
            store: Store::new(dir, limits),
            marks: Marks::new(dir),
            journal: Journal::new(dir),
            history: History::at(dir, memory),
            seen: Seen::at(dir),
            inputs,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::cache::hash::Hasher;

    pub(super) struct Dir(pub(super) PathBuf);

    impl Dir {
        pub(super) fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("farm-cache-{name}-{}", std::process::id()));
            std::fs::remove_dir_all(&dir).ok();
            Self(dir)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    pub(super) fn facts(words: &[String], digest: u128) -> Facts<'_> {
        Facts {
            job: "test",
            words,
            env: &[],
            digest: format!("{digest:032x}").parse().unwrap(),
            arch: Some("x86_64"),
            channel: Some("1.100.0"),
            config: Hasher::default().finish(),
        }
    }

    pub(super) fn entry(at: u64) -> Entry {
        Entry {
            job: "test".to_owned(),
            command: "cargo xtask check test".to_owned(),
            id: "petal.1a".to_owned(),
            host: "petal".to_owned(),
            workspace: "w".to_owned(),
            at: Moment(at),
            ran: 30,
            log: PathBuf::from("/x/log"),
            digest: Hasher::default().finish(),
        }
    }
}
