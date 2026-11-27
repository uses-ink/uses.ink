//! Which crates a change touches, and the one line that says so.

use std::collections::BTreeSet;
use std::fmt;

use crate::affected::graph::{Graph, Package};
use crate::affected::touched::Touched;
use crate::cache::inputs::glob::Selection;

/// Why every crate is affected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reason {
    /// A file every crate depends on changed: the lock file, the toolchain, a root manifest.
    Shared(Touched),
    /// A path in no crate that the repo does not call irrelevant: it may be read by any test.
    Unowned(Touched),
}

/// The crates to test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Choice {
    /// Nothing that matters changed.
    Nothing,
    /// Every crate.
    Everything { why: Reason, total: usize },
    /// The crates that own a changed path, and those that depend on them.
    Some { changed: Vec<Package>, dependents: Vec<Package>, total: usize },
}

impl Choice {
    /// How many names the line lists before it counts the rest.
    const SHOWN: usize = 6;

    /// What the changed paths affect. A path in a crate affects that crate and its dependents;
    /// one in no crate affects every crate unless `ignore` selects it; a shared one always does.
    pub(crate) fn of(graph: &Graph, touched: &[Touched], ignore: &Selection) -> Self {
        let total = graph.len();
        let mut seeds = BTreeSet::new();
        for path in touched {
            if path.shared() {
                return Self::Everything { why: Reason::Shared(path.clone()), total };
            }
            match graph.owner(path.as_str()) {
                Some(owner) => {
                    seeds.insert(owner);
                }
                None if ignore.selects(path.as_str()) => {}
                None => return Self::Everything { why: Reason::Unowned(path.clone()), total },
            }
        }
        if seeds.is_empty() {
            return Self::Nothing;
        }
        let dependents: BTreeSet<usize> = graph.closure(&seeds).difference(&seeds).copied().collect();
        Self::Some { changed: graph.names(&seeds), dependents: graph.names(&dependents), total }
    }

    /// The cargo flags that select these crates: `-p a -p b`, or `--workspace`; none for nothing.
    pub(crate) fn flags(&self) -> Option<Vec<String>> {
        match self {
            Self::Nothing => None,
            Self::Everything { .. } => Some(vec!["--workspace".to_owned()]),
            Self::Some { changed, dependents, .. } => {
                Some(changed.iter().chain(dependents).flat_map(|name| ["-p".to_owned(), name.to_string()]).collect())
            }
        }
    }

    fn list(names: &[Package]) -> String {
        let shown: Vec<String> = names.iter().take(Self::SHOWN).map(ToString::to_string).collect();
        match names.len().checked_sub(Self::SHOWN).filter(|more| *more > 0) {
            Some(more) => format!("{}, and {more} more", shown.join(", ")),
            None => shown.join(", "),
        }
    }
}

impl fmt::Display for Choice {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nothing => write!(out, "no crate affected"),
            Self::Everything { why: Reason::Shared(path), total } => {
                write!(out, "affected: all {total} crates (`{path}` is shared by every crate)")
            }
            Self::Everything { why: Reason::Unowned(path), total } => {
                write!(out, "affected: all {total} crates (`{path}` is in no crate)")
            }
            Self::Some { changed, dependents, total } => write!(
                out,
                "affected: {} of {total} crates (changed: {}; dependents: {})",
                changed.len() + dependents.len(),
                Self::list(changed),
                dependents.len()
            ),
        }
    }
}
