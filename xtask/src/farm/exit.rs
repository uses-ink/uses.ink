//! The exit codes of `cargo xtask farm`: typed, one meaning each, documented in
//! docs/workflow.md.

use std::fmt;
use std::process::ExitCode;

/// How a farm command ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Exit {
    /// It did what was asked: the command passed.
    Passed,
    /// The command ran and failed: a test, a lint, a compile error.
    Failed,
    /// The command line is wrong.
    Usage,
    /// The farm could not do it: no config, an unreachable host, a failed sync.
    Farm,
    /// The farm will not do it: a golden job on a host that is not golden, a workspace with work in it.
    Refused,
    /// The job is still running after the time given; call `farm wait` again.
    Running,
    /// The command hit its hard limit and was stopped.
    TimedOut,
}

impl Exit {
    /// Every code, in the order docs/workflow.md lists them.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 7] =
        [Self::Passed, Self::Failed, Self::Usage, Self::Farm, Self::Refused, Self::Running, Self::TimedOut];

    /// The process exit status.
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::Passed => 0,
            Self::Failed => 1,
            Self::Usage => 2,
            Self::Farm => 3,
            Self::Refused => 4,
            Self::Running => 75,
            Self::TimedOut => 124,
        }
    }

    /// The exit of a command that ended with `code`: `timeout`'s 124 is the limit, anything else
    /// but zero is a failure.
    pub(crate) const fn of(code: i32) -> Self {
        match code {
            0 => Self::Passed,
            124 => Self::TimedOut,
            _ => Self::Failed,
        }
    }

    /// The worse of two: the one a whole run is reported as when its parts differ. Lower codes
    /// of the farm's own trouble hide a failed job, which hides a pass.
    pub(crate) fn worst(self, other: Self) -> Self {
        let rank = |exit: Self| match exit {
            Self::Passed => 0,
            Self::Running => 1,
            Self::Failed => 2,
            Self::TimedOut => 3,
            Self::Refused => 4,
            Self::Usage => 5,
            Self::Farm => 6,
        };
        if rank(other) > rank(self) { other } else { self }
    }
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        Self::from(exit.code())
    }
}

impl fmt::Display for Exit {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Usage => "usage",
            Self::Farm => "farm",
            Self::Refused => "refused",
            Self::Running => "running",
            Self::TimedOut => "timed out",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_distinct_and_a_command_status_maps_to_one() {
        let codes: Vec<u8> = Exit::ALL.iter().map(|exit| exit.code()).collect();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), codes.len());
        assert_eq!(Exit::of(0), Exit::Passed);
        assert_eq!(Exit::of(1), Exit::Failed);
        assert_eq!(Exit::of(101), Exit::Failed);
        assert_eq!(Exit::of(124), Exit::TimedOut);
        assert_eq!(Exit::of(143), Exit::Failed);
        assert_eq!(Exit::of(-1), Exit::Failed);
    }

    #[test]
    fn the_worst_of_two_is_symmetric_and_a_pass_never_hides_a_failure() {
        for a in Exit::ALL {
            for b in Exit::ALL {
                assert_eq!(a.worst(b), b.worst(a), "{a} {b}");
            }
            assert_eq!(Exit::Passed.worst(a), a);
        }
        assert_eq!(Exit::Failed.worst(Exit::Running), Exit::Failed);
        assert_eq!(Exit::Failed.worst(Exit::Farm), Exit::Farm);
    }
}
