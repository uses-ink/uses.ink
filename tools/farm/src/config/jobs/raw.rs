//! The job file as the toml reads it, before it is checked.

use std::fmt;

use std::num::{NonZeroU32, NonZeroU64};

use serde::Deserialize;

use crate::affected::{Base, Scope};
use crate::cache::hash::Hasher;
use crate::cache::inputs::glob::Selection;
use crate::cli::name::{Disposable, Excluded};
use crate::config::jobs::{Fault, Limits, Line, Settings};
use crate::place::history::Memory;

/// The file as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Raw {
    #[serde(default)]
    pub(super) job: toml::Table,
    #[serde(default)]
    pub(super) workspace: Area,
    #[serde(default)]
    cache: Pool,
    #[serde(default)]
    budget: Spend,
    #[serde(default)]
    sync: Ship,
    #[serde(default)]
    history: Recall,
    #[serde(default)]
    affected: Reach,
}

impl Raw {
    /// What the tables beyond the jobs say, over the defaults.
    pub(super) fn settings(&mut self, text: &str) -> Result<Settings, Fault> {
        let (raw, defaults) = (self, Settings::default());
        let globs = raw.affected.ignore.take().unwrap_or_default();
        let ignore = Selection::parse(&globs).map_err(|(at, source)| Fault::Ignore {
            at: Line::first(text, globs.get(at).map_or("ignore", String::as_str)),
            source,
        })?;
        Ok(Settings {
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
            scope: Scope { base: raw.affected.base.take(), ignore },
        })
    }
}

/// The `[affected]` table.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reach {
    base: Option<Base>,
    ignore: Option<Vec<String>>,
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
pub(super) struct Area {
    #[serde(default)]
    pub(super) disposable: Vec<Disposable>,
}

/// One `[job.<name>]` table.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Entry {
    pub(super) run: Run,
    pub(super) golden: bool,
    pub(super) limit: Option<NonZeroU64>,
    pub(super) max_load: Option<NonZeroU32>,
    pub(super) inputs: Option<Vec<String>>,
    pub(super) cache: Option<bool>,
    pub(super) affected: Option<bool>,
}

/// The `run` key: a command of at least one word.
#[derive(Debug, Deserialize)]
#[serde(try_from = "String")]
pub(super) struct Run(pub(super) Vec<String>);

impl TryFrom<String> for Run {
    type Error = Blank;

    fn try_from(given: String) -> Result<Self, Blank> {
        let words: Vec<String> = given.split_whitespace().map(str::to_owned).collect();
        if words.is_empty() { Err(Blank) } else { Ok(Self(words)) }
    }
}

/// A `run` with no word in it.
#[derive(Debug)]
pub(super) struct Blank;

impl fmt::Display for Blank {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`run` is empty")
    }
}

impl std::error::Error for Blank {}
