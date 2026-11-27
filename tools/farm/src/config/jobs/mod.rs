//! The jobs a repo declares in its own `.config/farm.toml`: what `farm check` runs, so the farm
//! itself knows no stage of any project.
//!
//! ```toml
//! [job.lint]
//! run = "cargo xtask check lint"
//! golden = false
//!
//! [job.test]
//! run = "cargo xtask check test"
//! golden = true
//! limit = 900          # optional: the job's hard limit in seconds
//! max_load = 6         # optional: place it only on a host whose load average is at most this
//!
//! inputs = ["**", "!docs/**"]  # optional: the files the job reads; default the whole tree
//! cache = false        # optional: never reuse a pass of this job
//! affected = true      # optional: `farm check <job> --affected` runs nextest for the affected crates
//!
//! [workspace]           # optional
//! disposable = [".ink"] # names of files and directories `farm workspace --forget` may delete
//!
//! [sync]                # optional
//! exclude = [".jj", ".git"]   # names the sync leaves out, beside the built-in ones
//!
//! [cache]               # optional: how long and how many passes are kept
//! max_age_hours = 168
//! max_entries = 256
//!
//! [budget]              # optional: warn after this many full `farm check` runs a day
//! full_checks_per_day = 3
//!
//! [affected]            # optional: how `farm test` narrows to the crates a change affects
//! base = "main"         # the revision to measure from; default the workspace's fork from `main`
//! ignore = ["docs/**"]   # paths in no crate that no test reads, as globs
//!
//! [history]             # optional: how much timing history placement keeps
//! max_age_hours = 336
//! max_samples = 2000
//! ```

mod fault;
mod raw;
#[cfg(test)]
mod tests;

use std::num::{NonZeroU32, NonZeroU64};
use std::path::{Path, PathBuf};

use crate::affected::Scope;
use crate::cache::hash::{Hash, Hasher};
use crate::cache::inputs::glob::Selection;
use crate::cli::Seconds;
use crate::cli::error::Error;
use crate::cli::name::{Disposable, Excluded, JobName};
use crate::host::Load;
use crate::place::Need;
use crate::place::history::Memory;

pub(crate) use fault::{Fault, Line};
use raw::{Entry, Raw};

/// Where the repo's job file is, below the workspace root.
const FILE: [&str; 2] = [".config", "farm.toml"];

/// One declared job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Recipe {
    pub(crate) name: JobName,
    /// The command, split on white space: no shell reads it.
    pub(crate) words: Vec<String>,
    pub(crate) need: Need,
    pub(crate) limit: Option<Seconds>,
    /// The most load a host may carry for the job to be placed on it.
    pub(crate) ceiling: Option<Load>,
    /// The files the job reads; none means the whole tree.
    pub(crate) inputs: Option<Selection>,
    /// Whether a pass of it may be reused.
    pub(crate) cache: bool,
    /// Whether `farm check <job> --affected` may run it narrowed to the affected crates.
    pub(crate) narrow: bool,
}

/// How many passes are kept, and for how long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Limits {
    pub(crate) hours: NonZeroU64,
    pub(crate) entries: NonZeroU32,
}

impl Limits {
    /// A week.
    const HOURS: NonZeroU64 = NonZeroU64::MIN.saturating_add(167);
    const ENTRIES: NonZeroU32 = NonZeroU32::MIN.saturating_add(255);

    /// The age after which a pass is forgotten.
    pub(crate) const fn age(self) -> std::time::Duration {
        std::time::Duration::from_secs(self.hours.get() * 3600)
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self { hours: Self::HOURS, entries: Self::ENTRIES }
    }
}

/// What the job file says beyond the jobs: how the farm behaves in this repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Settings {
    pub(crate) limits: Limits,
    /// How much timing history placement keeps.
    pub(crate) memory: Memory,
    /// Full checks a day before `farm check` warns; none is off.
    pub(crate) budget: Option<NonZeroU32>,
    /// Names the sync leaves out and a digest does not read, beside the built-in ones.
    pub(crate) exclude: Vec<Excluded>,
    /// The digest of the job file's text: a changed job file is a new key for every job.
    pub(crate) config: Hash,
    /// How `farm test` narrows: the base revision and the paths no test reads.
    pub(crate) scope: Scope,
}

impl Settings {
    /// The names kept out of a sync when the file does not say.
    const EXCLUDE: [&'static str; 2] = [".jj", ".git"];
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            memory: Memory::default(),
            budget: None,
            exclude: Self::EXCLUDE.iter().filter_map(|name| name.parse().ok()).collect(),
            config: Hasher::default().finish(),
            scope: Scope::default(),
        }
    }
}

/// Every declared job, in the order the file lists them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Jobs {
    path: PathBuf,
    all: Vec<Recipe>,
    disposable: Vec<Disposable>,
    settings: Settings,
}

impl Jobs {
    /// Where the job file of the workspace at `root` is.
    pub(crate) fn path(root: &Path) -> PathBuf {
        FILE.iter().fold(root.to_path_buf(), |path, part| path.join(part))
    }

    /// Reads the job file of the workspace at `root`.
    pub(crate) fn load(root: &Path) -> Result<Self, Error> {
        let path = Self::path(root);
        let text = std::fs::read_to_string(&path)
            .map_err(|source| Error::Jobs { path: path.clone(), fault: Fault::Read(source) })?;
        Self::parse(&text, path.clone()).map_err(|fault| Error::Jobs { path, fault })
    }

    /// Reads the job file when there is one: a repo without it has no jobs and the defaults.
    pub(crate) fn load_optional(root: &Path) -> Result<Option<Self>, Error> {
        let path: PathBuf = FILE.iter().fold(root.to_path_buf(), |path, part| path.join(part));
        match path.try_exists() {
            Ok(false) => Ok(None),
            Ok(true) | Err(_) => Self::load(root).map(Some),
        }
    }

    /// What the file says beyond the jobs.
    pub(crate) const fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Reads a job file's text.
    pub(crate) fn parse(text: &str, path: PathBuf) -> Result<Self, Fault> {
        let mut raw: Raw = toml::from_str(text).map_err(|source| Fault::Toml(Box::new(source)))?;
        let settings = raw.settings(text)?;
        let mut recipes = Vec::new();
        for (key, value) in std::mem::take(&mut raw.job) {
            let name: JobName = key.parse().map_err(Fault::Name)?;
            let entry: Entry =
                value.try_into().map_err(|source| Fault::Entry { name: name.clone(), source: Box::new(source) })?;
            let inputs = match &entry.inputs {
                None => None,
                Some(globs) if globs.is_empty() => {
                    return Err(Fault::NoInputs { at: Line::of(text, &name, "inputs"), name });
                }
                Some(globs) => Some(Selection::parse(globs).map_err(|(at, source)| Fault::Inputs {
                    at: Line::of(text, &name, globs.get(at).map_or("inputs", String::as_str)),
                    name: name.clone(),
                    source,
                })?),
            };
            recipes.push(Recipe {
                name,
                words: entry.run.0,
                need: if entry.golden { Need::Golden } else { Need::Any },
                limit: entry.limit.map(|limit| Seconds(limit.get())),
                ceiling: entry.max_load.map(|max| Load(max.get().saturating_mul(100))),
                inputs,
                cache: entry.cache.unwrap_or(true),
                narrow: entry.affected.unwrap_or(false),
            });
        }
        if recipes.is_empty() {
            return Err(Fault::Empty);
        }
        Ok(Self { path, all: recipes, disposable: raw.workspace.disposable, settings })
    }

    /// The names of files and directories, at any depth, that removing a workspace may lose.
    pub(crate) fn disposable(&self) -> &[Disposable] {
        &self.disposable
    }

    /// The jobs named, in the order given, or every job when none is named.
    pub(crate) fn pick(&self, names: &[JobName]) -> Result<Vec<Recipe>, Error> {
        if names.is_empty() {
            return Ok(self.all.clone());
        }
        names
            .iter()
            .map(|name| {
                self.all.iter().find(|recipe| recipe.name == *name).cloned().ok_or_else(|| Error::Jobs {
                    path: self.path.clone(),
                    fault: Fault::Unknown {
                        given: name.clone(),
                        known: self.all.iter().map(|recipe| recipe.name.clone()).collect(),
                    },
                })
            })
            .collect()
    }
}
