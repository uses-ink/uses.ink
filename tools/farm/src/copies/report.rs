//! What `--forget` and `--prune` say about the copies.

use std::fmt;

use crate::cli::error::Error;
use crate::cli::exit::Exit;
use crate::cli::name::HostName;
use crate::copies::{Outcome, Reached, Stray};
use crate::job::event::Gone;

/// A size in kibibytes, as a person reads it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Size(pub(crate) u64);

impl fmt::Display for Size {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        const MIB: u64 = 1 << 10;
        const GIB: u64 = 1 << 20;
        match self.0 {
            kib if kib >= GIB => write!(out, "{}.{} GiB", kib / GIB, kib % GIB * 10 / GIB),
            kib if kib >= MIB => write!(out, "{} MiB", kib / MIB),
            kib => write!(out, "{kib} KiB"),
        }
    }
}

/// An error and its causes on one line.
struct Causes<'a>(&'a Error);

impl fmt::Display for Causes<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)?;
        let mut cause = std::error::Error::source(self.0);
        while let Some(error) = cause {
            write!(out, ": {error}")?;
            cause = error.source();
        }
        Ok(())
    }
}

/// What every host said about the copies it was asked to remove.
#[derive(Debug)]
pub(crate) struct Removal {
    reached: Vec<Reached>,
}

impl Removal {
    pub(crate) fn new(reached: Vec<Reached>) -> Self {
        Self { reached }
    }

    /// Whether no host was asked.
    pub(crate) fn is_empty(&self) -> bool {
        self.reached.is_empty()
    }

    /// Refused when a host would not remove a path it found outside its directory; a host that
    /// did not answer is said, and does not fail the command, since the workspace is gone here.
    pub(crate) fn exit(&self) -> Exit {
        let refused = self.reached.iter().any(|reached| matches!(reached.outcome, Outcome::Gone(Gone::Refused { .. })));
        if refused { Exit::Refused } else { Exit::Passed }
    }

    /// The kibibytes removed.
    fn freed(&self) -> u64 {
        self.reached
            .iter()
            .map(|reached| match reached.outcome {
                Outcome::Gone(Gone::Removed { kib }) => kib,
                _ => 0,
            })
            .sum()
    }
}

impl fmt::Display for Reached {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (host, path) = (&self.host, &self.path);
        match &self.outcome {
            Outcome::Gone(Gone::Removed { kib }) => write!(out, "{host}: removed {path} ({})", Size(*kib)),
            Outcome::Gone(Gone::Absent) => write!(out, "{host}: no copy at {path}"),
            Outcome::Gone(Gone::Refused { path: real }) => write!(
                out,
                "{host}: refused: {path} resolves to {real}, which is not directly inside the farm directory; nothing was removed"
            ),
            Outcome::Unreached(error) => write!(
                out,
                "{host}: not reached, so {path} may remain (`farm workspace --prune` removes it later): {}",
                Causes(error)
            ),
        }
    }
}

impl fmt::Display for Removal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, reached) in self.reached.iter().enumerate() {
            if i > 0 {
                writeln!(out)?;
            }
            write!(out, "{reached}")?;
        }
        if self.freed() > 0 {
            write!(out, "\nfreed {} on the hosts", Size(self.freed()))?;
        }
        Ok(())
    }
}

/// The copies no workspace here owns, and the hosts that could not be asked.
#[derive(Debug, Default)]
pub(crate) struct Survey {
    pub(crate) strays: Vec<Stray>,
    pub(crate) missed: Vec<(HostName, Error)>,
}

impl Survey {
    /// Whether there is nothing to remove.
    pub(crate) fn is_empty(&self) -> bool {
        self.strays.is_empty()
    }
}

impl fmt::Display for Survey {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for stray in &self.strays {
            writeln!(out, "{}: {} ({})", stray.host, stray.path, Size(stray.kib))?;
        }
        if self.strays.is_empty() {
            write!(out, "no host holds a copy that no workspace here owns")?;
        } else {
            let total = self.strays.iter().map(|stray| stray.kib).sum();
            write!(out, "{} copies, {}, that no workspace here owns", self.strays.len(), Size(total))?;
        }
        for (host, error) in &self.missed {
            write!(out, "\n{host}: not reached, its copies were not looked at: {}", Causes(error))?;
        }
        Ok(())
    }
}
