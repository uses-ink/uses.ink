//! What each workspace ran, for `farm status` and the budget.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::cache::{Fault, Moment};

/// What a journal line records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    /// A full `farm check`; `count` is the stages it ran, `secs` their host time.
    Check,
    /// A job run on a host; `secs` is its time there.
    Stage,
    /// A job served from the passes.
    Hit,
    /// A job attached to an identical one already running.
    Attach,
}

/// One line of the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub(crate) at: Moment,
    pub(crate) workspace: String,
    pub(crate) kind: Kind,
    pub(crate) job: String,
    pub(crate) secs: u64,
    pub(crate) count: u32,
}

/// What one workspace did in a day.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Tally {
    pub(crate) workspace: String,
    pub(crate) checks: u32,
    /// Full checks that ran at least one stage: the heavy ones.
    pub(crate) heavy: u32,
    pub(crate) stages: u32,
    pub(crate) hits: u32,
    pub(crate) attached: u32,
    pub(crate) secs: u64,
}

impl fmt::Display for Tally {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "{}: {} full checks, {} stage runs, {} cache hits, {} deduped, {} host seconds",
            self.workspace, self.checks, self.stages, self.hits, self.attached, self.secs
        )
    }
}

/// What each workspace ran.
#[derive(Debug, Clone)]
pub(crate) struct Journal {
    file: PathBuf,
}

impl Journal {
    /// A week is kept; `farm status` shows a day.
    const KEEP: Duration = Duration::from_hours(168);
    const DAY: Duration = Duration::from_hours(24);
    /// The size past which an append also drops what is older than a week.
    const LARGE: u64 = 256 * 1024;

    pub(crate) fn new(dir: &Path) -> Self {
        Self { file: dir.join("journal.jsonl") }
    }

    pub(crate) fn append(&self, record: &Record) -> Result<(), Fault> {
        use std::io::Write as _;
        let io = |source| Fault::Io { path: self.file.clone(), source };
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let line = serde_json::to_string(record).map_err(|source| Fault::Json { path: self.file.clone(), source })?;
        let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&self.file).map_err(io)?;
        writeln!(file, "{line}").map_err(io)?;
        if file.metadata().map_err(io)?.len() > Self::LARGE {
            self.trim(record.at)?;
        }
        Ok(())
    }

    fn read(&self) -> Result<Vec<Record>, Fault> {
        let text = match std::fs::read_to_string(&self.file) {
            Ok(text) => text,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => return Err(Fault::Io { path: self.file.clone(), source }),
        };
        // a line cut short by a crash is skipped: the journal is a count, not a record of truth
        Ok(text.lines().filter_map(|line| serde_json::from_str(line).ok()).collect())
    }

    fn trim(&self, now: Moment) -> Result<(), Fault> {
        let mut text = String::new();
        for record in self.read()?.into_iter().filter(|record| record.at.age(now) <= Self::KEEP) {
            text.push_str(
                &serde_json::to_string(&record).map_err(|source| Fault::Json { path: self.file.clone(), source })?,
            );
            text.push('\n');
        }
        std::fs::write(&self.file, text).map_err(|source| Fault::Io { path: self.file.clone(), source })
    }

    /// The last day, one tally per workspace, by name.
    pub(crate) fn tally(&self, now: Moment) -> Result<Vec<Tally>, Fault> {
        let mut tallies: Vec<Tally> = Vec::new();
        for record in self.read()?.into_iter().filter(|record| record.at.age(now) <= Self::DAY) {
            let at = if let Some(at) = tallies.iter().position(|tally| tally.workspace == record.workspace) {
                at
            } else {
                tallies.push(Tally { workspace: record.workspace.clone(), ..Tally::default() });
                tallies.len() - 1
            };
            let Some(tally) = tallies.get_mut(at) else { continue };
            match record.kind {
                Kind::Check => {
                    tally.checks += 1;
                    tally.heavy += u32::from(record.count > 0);
                }
                Kind::Stage => {
                    tally.stages += 1;
                    tally.secs += record.secs;
                }
                Kind::Hit => tally.hits += 1,
                Kind::Attach => tally.attached += 1,
            }
        }
        tallies.sort_by(|one, other| one.workspace.cmp(&other.workspace));
        Ok(tallies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::tests::Dir;

    #[test]
    fn the_journal_counts_a_day_per_workspace() {
        let dir = Dir::new("journal");
        let journal = Journal::new(&dir.0);
        let record = |workspace: &str, kind, at, secs, count| Record {
            at: Moment(at),
            workspace: workspace.to_owned(),
            kind,
            job: "j".to_owned(),
            secs,
            count,
        };
        let day = 86_400;
        let now = 10 * day;
        for line in [
            record("a", Kind::Check, now - 10, 0, 2),
            record("a", Kind::Check, now - 20, 0, 0),
            record("a", Kind::Stage, now - 10, 30, 0),
            record("a", Kind::Stage, now - 10, 12, 0),
            record("a", Kind::Hit, now - 5, 0, 0),
            record("b", Kind::Attach, now - 5, 0, 0),
            record("a", Kind::Stage, now - day - 1, 99, 0),
        ] {
            journal.append(&line).unwrap();
        }
        let tallies = journal.tally(Moment(now)).unwrap();
        assert_eq!(tallies.len(), 2);
        let a = tallies.first().unwrap();
        assert_eq!((a.checks, a.heavy, a.stages, a.hits, a.attached, a.secs), (2, 1, 2, 1, 0, 42));
        assert_eq!(a.to_string(), "a: 2 full checks, 2 stage runs, 1 cache hits, 0 deduped, 42 host seconds");
        assert_eq!(tallies.get(1).unwrap().attached, 1);
    }
}
