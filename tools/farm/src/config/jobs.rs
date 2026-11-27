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
//!
//! inputs = ["**", "!docs/**"]  # optional: the files the job reads; default the whole tree
//! cache = false        # optional: never reuse a pass of this job
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
//! [history]             # optional: how much timing history placement keeps
//! max_age_hours = 336
//! max_samples = 2000
//! ```

use std::fmt;
use std::num::{NonZeroU32, NonZeroU64};
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::cache::hash::{Hash, Hasher};
use crate::cache::inputs::glob::{self, Selection};
use crate::cli::Seconds;
use crate::cli::error::{Error, Listed};
use crate::cli::name::{Disposable, Excluded, JobName};
use crate::place::Need;
use crate::place::history::Memory;

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
    /// The files the job reads; none means the whole tree.
    pub(crate) inputs: Option<Selection>,
    /// Whether a pass of it may be reused.
    pub(crate) cache: bool,
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
    /// Reads the job file of the workspace at `root`.
    pub(crate) fn load(root: &Path) -> Result<Self, Error> {
        let path: PathBuf = FILE.iter().fold(root.to_path_buf(), |path, part| path.join(part));
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
        let settings = raw.settings(text);
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
                inputs,
                cache: entry.cache.unwrap_or(true),
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

/// The file as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    #[serde(default)]
    job: toml::Table,
    #[serde(default)]
    workspace: Area,
    #[serde(default)]
    cache: Pool,
    #[serde(default)]
    budget: Spend,
    #[serde(default)]
    sync: Ship,
    #[serde(default)]
    history: Recall,
}

impl Raw {
    /// What the tables beyond the jobs say, over the defaults.
    fn settings(&mut self, text: &str) -> Settings {
        let (raw, defaults) = (self, Settings::default());
        Settings {
            limits: Limits {
                hours: raw.cache.max_age_hours.unwrap_or(defaults.limits.hours),
                entries: raw.cache.max_entries.unwrap_or(defaults.limits.entries),
            },
            memory: Memory {
                hours: raw.history.max_age_hours.unwrap_or(defaults.memory.hours),
                samples: raw.history.max_samples.unwrap_or(defaults.memory.samples),
            },
            budget: raw.budget.full_checks_per_day,
            exclude: raw.sync.exclude.take().unwrap_or(defaults.exclude),
            config: Hasher::default().field(text.as_bytes()).finish(),
        }
    }
}

/// The `[history]` table.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Recall {
    max_age_hours: Option<NonZeroU64>,
    max_samples: Option<NonZeroU32>,
}

/// The `[cache]` table.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pool {
    max_age_hours: Option<NonZeroU64>,
    max_entries: Option<NonZeroU32>,
}

/// The `[budget]` table.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Spend {
    full_checks_per_day: Option<NonZeroU32>,
}

/// The `[sync]` table.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ship {
    exclude: Option<Vec<Excluded>>,
}

/// The `[workspace]` table.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Area {
    #[serde(default)]
    disposable: Vec<Disposable>,
}

/// One `[job.<name>]` table.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    run: Run,
    golden: bool,
    limit: Option<NonZeroU64>,
    inputs: Option<Vec<String>>,
    cache: Option<bool>,
}

/// The `run` key: a command of at least one word.
#[derive(Debug, Deserialize)]
#[serde(try_from = "String")]
struct Run(Vec<String>);

impl TryFrom<String> for Run {
    type Error = Blank;

    fn try_from(given: String) -> Result<Self, Blank> {
        let words: Vec<String> = given.split_whitespace().map(str::to_owned).collect();
        if words.is_empty() { Err(Blank) } else { Ok(Self(words)) }
    }
}

/// A `run` with no word in it.
#[derive(Debug)]
struct Blank;

impl fmt::Display for Blank {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`run` is empty")
    }
}

impl std::error::Error for Blank {}

/// A line of the job file, counted from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Line(pub(crate) usize);

impl Line {
    /// The line of `word` in the table of job `name`, else the table's own line, else the first.
    fn of(text: &str, name: &JobName, word: &str) -> Self {
        let header = format!("[job.{name}]");
        let mut table = None;
        for (at, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed == header {
                table = Some(at);
            } else if trimmed.starts_with('[') && table.is_some() {
                break;
            } else if table.is_some() && line.contains(word) {
                return Self(at + 1);
            }
        }
        Self(table.map_or(1, |at| at + 1))
    }
}

impl fmt::Display for Line {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "line {}", self.0)
    }
}

/// A job file that cannot be used, or a job it does not declare.
#[derive(Debug)]
pub(crate) enum Fault {
    Read(std::io::Error),
    Toml(Box<toml::de::Error>),
    /// A job name the farm does not allow.
    Name(crate::cli::name::Fault),
    /// A `[job.<name>]` table that is not one.
    Entry {
        name: JobName,
        source: Box<toml::de::Error>,
    },
    /// A glob of `inputs` that is not one.
    Inputs {
        name: JobName,
        at: Line,
        source: glob::Fault,
    },
    /// An `inputs = []`: it would read nothing.
    NoInputs {
        name: JobName,
        at: Line,
    },
    /// A file with no `[job.*]`.
    Empty,
    /// A job asked for that the file does not declare.
    Unknown {
        given: JobName,
        known: Vec<JobName>,
    },
}

impl Fault {
    /// Whether the person named a job that is not there: a usage error, not the farm's.
    pub(crate) const fn is_usage(&self) -> bool {
        matches!(self, Self::Unknown { .. })
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(_) => write!(out, "cannot be read"),
            Self::Toml(_) => write!(out, "is not the TOML expected"),
            Self::Name(_) => write!(out, "names a job that is not allowed"),
            Self::Entry { name, .. } => {
                write!(
                    out,
                    "has a `[job.{name}]` that is not `run`, `golden` and the optional `limit`, `inputs` and `cache`"
                )
            }
            Self::Inputs { name, at, .. } => {
                write!(out, "has an `inputs` glob of `[job.{name}]` that is not allowed, at {at}")
            }
            Self::NoInputs { name, at } => {
                write!(out, "has an empty `inputs` in `[job.{name}]` at {at}; leave it out to read the whole tree")
            }
            Self::Empty => write!(out, "declares no `[job.<name>]`"),
            Self::Unknown { given, known } => write!(out, "declares no job `{given}`; the jobs are {}", Listed(known)),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(source) => Some(source),
            Self::Toml(source) | Self::Entry { source, .. } => Some(source),
            Self::Name(source) => Some(source),
            Self::Inputs { source, .. } => Some(source),
            Self::Empty | Self::NoInputs { .. } | Self::Unknown { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::error::Error;

    const FILE: &str = "[job.lint]\nrun = \"cargo xtask check lint\"\ngolden = false\n\n[job.test]\nrun = \"cargo  xtask check test\"\ngolden = true\nlimit = 900\n";

    #[test]
    fn jobs_come_in_file_order_with_their_need_and_limit() {
        let jobs = Jobs::parse(FILE, PathBuf::new()).unwrap();
        let all = jobs.pick(&[]).unwrap();
        assert_eq!(all.iter().map(|recipe| recipe.name.as_str()).collect::<Vec<_>>(), ["lint", "test"]);
        assert_eq!(all[1].words, ["cargo", "xtask", "check", "test"]);
        assert_eq!((all[0].need, all[1].need), (Need::Any, Need::Golden));
        assert_eq!((all[0].limit, all[1].limit), (None, Some(Seconds(900))));
    }

    #[test]
    fn a_pick_keeps_the_order_asked_and_an_unknown_job_is_usage() {
        let jobs = Jobs::parse(FILE, PathBuf::new()).unwrap();
        let asked: Vec<JobName> = vec!["test".parse().unwrap()];
        assert_eq!(jobs.pick(&asked).unwrap().len(), 1);
        let nope = jobs.pick(&["nope".parse().unwrap()]).unwrap_err();
        assert_eq!(nope.exit(), crate::cli::exit::Exit::Usage);
        assert!(nope.to_string().contains("declares no job `nope`"), "{nope}");
    }

    #[test]
    fn this_repos_file_parses_and_has_golden_and_plain_jobs() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let all = Jobs::load(&root).unwrap().pick(&[]).unwrap();
        assert!(all.iter().any(|job| job.need == Need::Golden) && all.iter().any(|job| job.need == Need::Any));
        assert!(all.iter().all(|job| job.words.first().is_some_and(|word| word == "cargo")));
    }

    #[test]
    fn the_workspace_table_declares_disposable_names() {
        let text = format!("{FILE}\n[workspace]\ndisposable = [\".ink\", \"node_modules\"]\n");
        let jobs = Jobs::parse(&text, PathBuf::new()).unwrap();
        assert_eq!(jobs.disposable().iter().map(Disposable::as_str).collect::<Vec<_>>(), [".ink", "node_modules"]);
        assert_eq!(Jobs::parse(FILE, PathBuf::new()).unwrap().disposable(), &[] as &[Disposable]);
        for bad in [
            "disposable = [\"a/b\"]",
            "disposable = [\"..\"]",
            "disposable = [\"\"]",
            "disposable = [\"a b\"]",
            "keep = []",
        ] {
            let text = format!("{FILE}\n[workspace]\n{bad}\n");
            assert!(Jobs::parse(&text, PathBuf::new()).is_err(), "{bad}");
        }
    }

    #[test]
    fn this_repo_declares_its_ink_cache_disposable() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        assert!(Jobs::load(&root).unwrap().disposable().iter().any(|name| name.as_str() == ".ink"));
    }

    #[test]
    fn inputs_cache_and_the_settings_tables_are_read_with_their_defaults() {
        let text = format!(
            "{FILE}\n[job.docs]\nrun = \"x\"\ngolden = false\ninputs = [\"**\", \"!docs/**\"]\ncache = false\n\n[cache]\nmax_age_hours = 2\nmax_entries = 5\n\n[budget]\nfull_checks_per_day = 3\n\n[sync]\nexclude = [\".jj\", \".impeccable\"]\n"
        );
        let jobs = Jobs::parse(&text, PathBuf::new()).unwrap();
        let all = jobs.pick(&[]).unwrap();
        assert!(all[0].inputs.is_none() && all[0].cache, "the whole tree and cached by default");
        assert!(all[2].inputs.as_ref().is_some_and(|files| files.selects("a.rs") && !files.selects("docs/a.md")));
        assert!(!all[2].cache);
        let settings = jobs.settings();
        assert_eq!((settings.limits.hours.get(), settings.limits.entries.get()), (2, 5));
        assert_eq!(settings.budget.map(NonZeroU32::get), Some(3));
        assert_eq!(
            settings.exclude.iter().map(crate::cli::name::Word::as_str).collect::<Vec<_>>(),
            [".jj", ".impeccable"]
        );
        let plain = Jobs::parse(FILE, PathBuf::new()).unwrap();
        let defaults = plain.settings();
        assert_eq!((defaults.limits.hours.get(), defaults.limits.entries.get(), defaults.budget), (168, 256, None));
        assert_eq!(defaults.exclude.iter().map(crate::cli::name::Word::as_str).collect::<Vec<_>>(), [".jj", ".git"]);
        assert_ne!(defaults.config, settings.config, "a changed file is a new config digest");
    }

    #[test]
    fn a_bad_glob_names_the_job_and_the_line_of_the_file() {
        let text = "[job.lint]\nrun = \"x\"\ngolden = false\n\n[job.test]\nrun = \"y\"\ngolden = true\ninputs = [\n  \"**\",\n  \"src/[ab]\",\n]\n";
        let fault = Jobs::parse(text, PathBuf::from("/r/.config/farm.toml")).unwrap_err();
        let shown = fault.to_string();
        assert!(shown.contains("[job.test]") && shown.contains("line 10"), "{shown}");
        let cause = std::error::Error::source(&fault).unwrap().to_string();
        assert!(cause.contains("`src/[ab]`") && cause.contains('['), "{cause}");
        let error = Error::Jobs { path: PathBuf::from("/r/.config/farm.toml"), fault };
        assert!(error.to_string().contains("/r/.config/farm.toml"), "{error}");
        let empty = "[job.a]\nrun = \"x\"\ngolden = false\ninputs = []\n";
        assert!(Jobs::parse(empty, PathBuf::new()).unwrap_err().to_string().contains("line 4"));
        for bad in ["max_age_hours = 0", "max_entries = 0", "wat = 1"] {
            assert!(Jobs::parse(&format!("{FILE}\n[cache]\n{bad}\n"), PathBuf::new()).is_err(), "{bad}");
        }
        assert!(Jobs::parse(&format!("{FILE}\n[sync]\nexclude = [\"a/b\"]\n"), PathBuf::new()).is_err());
    }

    #[test]
    fn this_repos_inputs_cover_every_crate_of_the_jobs_that_read_the_whole_workspace() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
        let all = Jobs::load(&root).unwrap().pick(&[]).unwrap();
        let manifest: toml::Table = toml::from_str(&std::fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
        let members: Vec<String> = manifest["workspace"]["members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|member| member.as_str().unwrap().to_owned())
            .collect();
        assert!(members.len() > 10);
        for name in ["lint", "test", "docs", "sites"] {
            let job = all.iter().find(|job| job.name.as_str() == name).unwrap();
            for member in &members {
                let file = format!("{member}/src/lib.rs");
                let selected = job.inputs.as_ref().is_none_or(|files| files.selects(&file));
                assert!(selected, "{name} does not read {file}: a new crate must not be served a stale pass");
            }
        }
        let test = all.iter().find(|job| job.name.as_str() == "test").unwrap();
        assert!(
            test.inputs.as_ref().is_some_and(|files| files.selects("docs/workflow.md")),
            "the farm's own test reads it"
        );
    }

    #[test]
    fn a_bad_file_says_what_is_wrong() {
        for bad in [
            "",
            "[job.a]\nrun = \"\"\ngolden = true\n",
            "[job.a]\nrun = \"x\"\n",
            "[job.A]\nrun = \"x\"\ngolden = true\n",
            "[job.a]\nrun = \"x\"\ngolden = true\nlimit = 0\n",
            "[job.a]\nrun = \"x\"\ngolden = true\nextra = 1\n",
            "jobs = 1\n",
        ] {
            assert!(Jobs::parse(bad, PathBuf::new()).is_err(), "{bad:?}");
        }
    }
}
