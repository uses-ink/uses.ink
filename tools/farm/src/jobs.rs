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
//! [workspace]           # optional
//! disposable = [".ink"] # names of files and directories `farm workspace --forget` may delete
//! ```

use std::fmt;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::cli::Seconds;
use crate::error::{Error, Listed};
use crate::name::{Disposable, JobName};
use crate::place::Need;

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
}

/// Every declared job, in the order the file lists them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Jobs {
    path: PathBuf,
    all: Vec<Recipe>,
    disposable: Vec<Disposable>,
}

impl Jobs {
    /// Reads the job file of the workspace at `root`.
    pub(crate) fn load(root: &Path) -> Result<Self, Error> {
        let path: PathBuf = FILE.iter().fold(root.to_path_buf(), |path, part| path.join(part));
        let text = std::fs::read_to_string(&path)
            .map_err(|source| Error::Jobs { path: path.clone(), fault: Fault::Read(source) })?;
        Self::parse(&text, path.clone()).map_err(|fault| Error::Jobs { path, fault })
    }

    /// Reads a job file's text.
    pub(crate) fn parse(text: &str, path: PathBuf) -> Result<Self, Fault> {
        let raw: Raw = toml::from_str(text).map_err(|source| Fault::Toml(Box::new(source)))?;
        let mut recipes = Vec::new();
        for (key, value) in raw.job {
            let name: JobName = key.parse().map_err(Fault::Name)?;
            let entry: Entry =
                value.try_into().map_err(|source| Fault::Entry { name: name.clone(), source: Box::new(source) })?;
            recipes.push(Recipe {
                name,
                words: entry.run.0,
                need: if entry.golden { Need::Golden } else { Need::Any },
                limit: entry.limit.map(|limit| Seconds(limit.get())),
            });
        }
        if recipes.is_empty() {
            return Err(Fault::Empty);
        }
        Ok(Self { path, all: recipes, disposable: raw.workspace.disposable })
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

/// A job file that cannot be used, or a job it does not declare.
#[derive(Debug)]
pub(crate) enum Fault {
    Read(std::io::Error),
    Toml(Box<toml::de::Error>),
    /// A job name the farm does not allow.
    Name(crate::name::Fault),
    /// A `[job.<name>]` table that is not one.
    Entry {
        name: JobName,
        source: Box<toml::de::Error>,
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
                write!(out, "has a `[job.{name}]` that is not `run`, `golden` and an optional `limit`")
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
            Self::Empty | Self::Unknown { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(nope.exit(), crate::exit::Exit::Usage);
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
        assert!(Jobs::parse(FILE, PathBuf::new()).unwrap().disposable().is_empty());
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
