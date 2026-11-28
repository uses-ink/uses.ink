//! A job: a command with what it needs, and what became of it. [`Flow`] reads a running job's
//! output line by line; [`Finished`] is the result a verb prints, as text or as `uses.farm/v1`.

mod document;
pub(crate) mod event;
mod finished;
mod flow;
pub(crate) mod quote;
pub(crate) mod summary;

use crate::cache::Ago;
use crate::cache::inputs::glob::Selection;
use crate::cli::Seconds;
use crate::config::jobs::Recipe;
use crate::host::{Load, Snapshot};
use crate::place::Need;
use crate::place::history::Key;
pub(crate) use document::Document;
pub(crate) use finished::{Finished, Totals, Verdict};
pub(crate) use flow::{Flow, LogFile};
use std::time::Duration;

/// Whether a pass of a job may be reused, and which files it depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reuse {
    /// Never: the job changes files the caller wants, or the repo said `cache = false`.
    Never,
    /// Yes, keyed by the whole tree.
    Whole,
    /// Yes, keyed by the files these globs select.
    Files(Selection),
}

impl Reuse {
    /// The globs, when the job names its inputs.
    pub(crate) const fn files(&self) -> Option<&Selection> {
        match self {
            Self::Files(files) => Some(files),
            Self::Never | Self::Whole => None,
        }
    }

    pub(crate) const fn allowed(&self) -> bool {
        !matches!(self, Self::Never)
    }
}

/// How a result came to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Served {
    /// The job ran on a host for this call.
    Ran,
    /// A run with the same command and inputs passed earlier; nothing ran.
    Cached { ago: Ago },
    /// An identical job was already running; this call waited for it.
    Attached,
}

impl Served {
    /// The word `--json` uses.
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Ran => "ran",
            Self::Cached { .. } => "cached",
            Self::Attached => "attached",
        }
    }
}

/// A command to run on a host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Job {
    /// What it is called in the output: a stage, or the command.
    pub(crate) label: String,
    /// The command: `cargo`, then its arguments.
    pub(crate) words: Vec<String>,
    pub(crate) need: Need,
    /// Variables beyond the farm's own, as `NAME=value`; they win over them.
    pub(crate) env: Vec<String>,
    /// Its own hard limit, when the repo declares one.
    pub(crate) limit: Option<Seconds>,
    /// The most load a host may carry for the job to be placed on it; none is any.
    pub(crate) ceiling: Option<Load>,
    pub(crate) reuse: Reuse,
    /// What its timings are filed under.
    pub(crate) key: Key,
}

impl From<Recipe> for Job {
    fn from(recipe: Recipe) -> Self {
        Self {
            key: Key::stage(recipe.name.as_str()),
            label: recipe.name.to_string(),
            words: recipe.words,
            need: recipe.need,
            env: Vec::new(),
            limit: recipe.limit,
            ceiling: recipe.ceiling,
            reuse: match (recipe.cache, recipe.inputs) {
                (false, _) => Reuse::Never,
                (true, None) => Reuse::Whole,
                (true, Some(files)) => Reuse::Files(files),
            },
        }
    }
}

impl Job {
    /// `cargo <args>`, every argument as given, a `--` and what follows it included.
    pub(crate) fn cargo(args: &[String]) -> Self {
        let mut words = vec!["cargo".to_owned()];
        words.extend(args.iter().cloned());
        Self {
            label: words.join(" "),
            need: Need::of_cargo(args),
            words,
            env: Vec::new(),
            limit: None,
            ceiling: None,
            reuse: Reuse::of_cargo(args),
            key: Key::cargo(args),
        }
    }

    /// The command as it is shown.
    pub(crate) fn shown(&self) -> String {
        self.words.join(" ")
    }

    /// The environment every farm run of `snap`'s host gets, the same every time so the host's
    /// `target/` stays warm: smaller debug info (line tables keep backtraces and panics readable,
    /// and the build and the link are faster), no colour, and the cores of one slot for cargo
    /// and for the test threads. `CARGO_TARGET_DIR` is never set.
    pub(crate) fn vars(&self, snap: &Snapshot) -> Vec<String> {
        self.vars_with(snap, std::env::var("NEXTEST_PROFILE").ok().as_deref())
    }

    /// [`Self::vars`] with the nextest profile the caller set, if any. The farm's hosts are
    /// shared, so a job runs the `ci` profile (the 100 ms budgets are for a quiet machine) unless
    /// the caller chose one.
    pub(crate) fn vars_with(&self, snap: &Snapshot, caller: Option<&str>) -> Vec<String> {
        let (share, threads) = (snap.share(), snap.threads());
        let mut vars = vec![
            "CARGO_PROFILE_DEV_DEBUG=line-tables-only".to_owned(),
            "CARGO_TERM_COLOR=never".to_owned(),
            format!("CARGO_BUILD_JOBS={share}"),
            format!("NEXTEST_TEST_THREADS={threads}"),
            "INSTA_UPDATE=no".to_owned(),
        ];
        if caller.is_none_or(str::is_empty) {
            vars.push("NEXTEST_PROFILE=ci".to_owned());
        }
        vars.extend(self.env.iter().cloned());
        vars
    }
}

/// How a call to run a job waits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Attach {
    /// Follow the job for at most this long, then let it go on without the client.
    Hold(Duration),
    /// Start it and leave.
    Detach,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_environment_is_the_same_every_time_and_never_sets_the_target_dir() {
        let job = Job {
            label: "x".to_owned(),
            words: vec!["cargo".to_owned()],
            need: Need::Any,
            env: vec!["INSTA_UPDATE=always".to_owned()],
            limit: None,
            ceiling: None,
            key: crate::place::history::Key::stage("x"),
            reuse: Reuse::Never,
        };
        let snap = crate::host::Snapshot::read(
            &crate::host::Host::local(),
            "now 1\narch x86_64\ncores 14\nload 0.1\nslots 2\nslot 0 free\nslot 1 free\nqueued 0\n",
        )
        .unwrap();
        let vars = job.vars_with(&snap, None);
        assert!(vars.contains(&"CARGO_BUILD_JOBS=7".to_owned()));
        assert!(vars.contains(&"NEXTEST_TEST_THREADS=7".to_owned()));
        assert!(vars.contains(&"CARGO_PROFILE_DEV_DEBUG=line-tables-only".to_owned()));
        assert_eq!(
            vars.last().map(String::as_str),
            Some("INSTA_UPDATE=always"),
            "the job's own variable comes last and wins"
        );
        assert!(!vars.iter().any(|var| var.starts_with("CARGO_TARGET_DIR")));
        assert_eq!(vars, job.vars_with(&snap, Some("")), "an empty profile counts as none");
        assert!(vars.contains(&"NEXTEST_PROFILE=ci".to_owned()), "shared hosts run the ci profile");
        let chosen = job.vars_with(&snap, Some("default"));
        assert!(!chosen.iter().any(|var| var.starts_with("NEXTEST_PROFILE")), "the caller's choice is left alone");
    }

    #[test]
    fn a_hosts_test_thread_cap_lowers_nextests_threads_and_never_raises_them() {
        let job = Job {
            label: "x".to_owned(),
            words: vec!["cargo".to_owned()],
            need: Need::Any,
            env: vec![],
            limit: None,
            ceiling: None,
            key: crate::place::history::Key::stage("x"),
            reuse: Reuse::Never,
        };
        let probe = "now 1\narch x86_64\ncores 16\nload 0.1\nslots 2\nslot 0 free\nslot 1 free\nqueued 0\n";
        let capped = |cap| {
            let host = crate::host::Host { test_threads: cap, ..crate::host::Host::local() };
            job.vars_with(&crate::host::Snapshot::read(&host, probe).unwrap(), Some("ci"))
        };
        assert!(capped(Some(4)).contains(&"NEXTEST_TEST_THREADS=4".to_owned()));
        assert!(capped(Some(12)).contains(&"NEXTEST_TEST_THREADS=8".to_owned()), "never above the share");
        assert!(capped(None).contains(&"NEXTEST_TEST_THREADS=8".to_owned()));
        assert!(capped(Some(4)).contains(&"CARGO_BUILD_JOBS=8".to_owned()), "cargo keeps its share");
    }
}
