//! What a program says about itself: a line `@@farm:<word> <numbers>` among a job's output.

use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;
use std::time::Duration;

use crate::farm::program::MARK;

/// One thing a program said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    /// A detached job was started.
    Started,
    /// The job is waiting; `behind` jobs are ahead of it.
    Queued { behind: u32 },
    /// The job took slot `slot` after waiting `queued`.
    Slot { slot: u32, queued: Duration },
    /// The command ended with `code` after running for `ran`.
    Done { code: i32, ran: Duration },
    /// The job was stopped before it ended: its client went away.
    Aborted,
    /// The client let the job outlive it.
    Detached,
    /// The job's body is gone and left no result.
    Lost,
    /// The job is still running after the time given.
    Running,
    /// There is no such job on the host.
    NoJob,
    /// The workspace has no directory on the host.
    NoDir,
    /// The job's body did not start.
    NoBody,
    /// The workspace's directory on the host was made, or was there.
    Seed(Seeded),
    /// Old job state removed.
    Swept(u32),
}

/// How a workspace's directory came to be on a host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Seeded {
    /// It was there already.
    Present,
    /// There was no sibling to start from; it is empty.
    Empty,
    /// Reflinked from the sibling named: seconds, not minutes.
    Reflink(String),
    /// Copied byte for byte from the sibling named: the filesystem has no reflinks.
    Copy(String),
}

/// A line of a program's output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Line {
    Event(Event),
    /// A line of the job's own output.
    Output(String),
}

impl Line {
    /// Reads one line: an event if it carries the mark, output otherwise.
    pub(crate) fn read(text: &str) -> Result<Self, Fault> {
        match text.strip_prefix(MARK) {
            Some(rest) => rest.parse().map(Self::Event),
            None => Ok(Self::Output(text.to_owned())),
        }
    }
}

/// A line that carries the mark and is not an event.
#[derive(Debug)]
pub(crate) enum Fault {
    Unknown { line: String },
    Number { line: String, source: ParseIntError },
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown { line } => write!(out, "`{line}` is not something a farm program says"),
            Self::Number { line, .. } => write!(out, "`{line}` holds a number that is not one"),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Number { source, .. } => Some(source),
            Self::Unknown { .. } => None,
        }
    }
}

impl FromStr for Event {
    type Err = Fault;

    fn from_str(line: &str) -> Result<Self, Fault> {
        let mut words = line.split_whitespace();
        let number = |word: Option<&str>| -> Result<i64, Fault> {
            let text = word.ok_or_else(|| Fault::Unknown { line: line.to_owned() })?;
            text.parse().map_err(|source| Fault::Number { line: line.to_owned(), source })
        };
        let seconds = |n: i64| Duration::from_secs(u64::try_from(n).unwrap_or(0));
        let count = |n: i64| u32::try_from(n).unwrap_or(0);
        let first = words.next();
        Ok(match first {
            Some("started") => Self::Started,
            Some("aborted") => Self::Aborted,
            Some("detached") => Self::Detached,
            Some("lost") => Self::Lost,
            Some("running") => Self::Running,
            Some("nojob") => Self::NoJob,
            Some("nodir") => Self::NoDir,
            Some("nobody") => Self::NoBody,
            Some("queued") => Self::Queued { behind: count(number(words.next())?) },
            Some("swept") => Self::Swept(count(number(words.next())?)),
            Some("slot") => {
                let slot = count(number(words.next())?);
                Self::Slot { slot, queued: seconds(number(words.next())?) }
            }
            Some("done") => {
                let code = i32::try_from(number(words.next())?).unwrap_or(i32::MAX);
                Self::Done { code, ran: seconds(number(words.next())?) }
            }
            Some("seed") => Self::Seed(match (words.next(), words.next()) {
                (Some("present"), _) => Seeded::Present,
                (Some("empty"), _) => Seeded::Empty,
                (Some("reflink"), Some(from)) => Seeded::Reflink(from.to_owned()),
                (Some("copy"), Some(from)) => Seeded::Copy(from.to_owned()),
                _ => return Err(Fault::Unknown { line: line.to_owned() }),
            }),
            _ => return Err(Fault::Unknown { line: line.to_owned() }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_read_back() {
        let table = [
            ("@@farm:started", Event::Started),
            ("@@farm:queued 3", Event::Queued { behind: 3 }),
            ("@@farm:slot 1 12", Event::Slot { slot: 1, queued: Duration::from_secs(12) }),
            ("@@farm:done 101 7", Event::Done { code: 101, ran: Duration::from_secs(7) }),
            ("@@farm:aborted", Event::Aborted),
            ("@@farm:seed reflink main", Event::Seed(Seeded::Reflink("main".to_owned()))),
            ("@@farm:seed copy a.b", Event::Seed(Seeded::Copy("a.b".to_owned()))),
            ("@@farm:swept 4", Event::Swept(4)),
        ];
        for (text, event) in table {
            assert_eq!(Line::read(text).unwrap(), Line::Event(event), "{text}");
        }
    }

    #[test]
    fn output_is_not_an_event_and_a_bad_event_is_an_error() {
        assert_eq!(Line::read("   Compiling x").unwrap(), Line::Output("   Compiling x".to_owned()));
        assert_eq!(Line::read("").unwrap(), Line::Output(String::new()));
        for bad in ["@@farm:", "@@farm:wat", "@@farm:slot x 1", "@@farm:done", "@@farm:seed reflink"] {
            assert!(Line::read(bad).is_err(), "{bad}");
        }
    }
}
