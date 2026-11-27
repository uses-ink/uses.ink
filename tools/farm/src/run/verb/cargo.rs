//! `farm cargo`: any cargo command on a placed host.
use crate::cli::error::Error;
use crate::host::transport::Transport;
use crate::job::{Job, Reuse};
use crate::place::{Need, Placement};
use crate::run::{Batch, Farm};

impl<T: Transport + Sync, P: Placement + Sync> Farm<T, P> {
    /// Any cargo command, on a placed host.
    pub(in crate::run) fn cargo(&self, args: &[String]) -> Result<Batch, Error> {
        let job = Job::cargo(args);
        let snaps = self.place(&[&job])?;
        Ok(self.fan(&[job], &snaps))
    }
}

impl Need {
    /// The cargo flags that take their value as the next word.
    pub(in crate::run) const VALUED: [&'static str; 21] = [
        "-C",
        "--config",
        "-Z",
        "-p",
        "--package",
        "--exclude",
        "-F",
        "--features",
        "--bin",
        "--test",
        "--bench",
        "--example",
        "--manifest-path",
        "-E",
        "--target",
        "--target-dir",
        "--profile",
        "-j",
        "--jobs",
        "--color",
        "--message-format",
    ];

    /// The cargo subcommand: the first word that is not a flag or a flag's value.
    pub(crate) fn subcommand(args: &[String]) -> Option<&str> {
        let mut words = args.iter().map(String::as_str);
        while let Some(word) = words.next() {
            if Self::VALUED.contains(&word) {
                words.next();
            } else if !word.starts_with('+') && !word.starts_with('-') {
                return Some(word);
            }
        }
        None
    }

    /// A cargo command needs a golden host when it runs tests: its verb is `test`, `nextest`,
    /// `xtask` or `insta`.
    pub(crate) fn of_cargo(args: &[String]) -> Self {
        match Self::subcommand(args) {
            Some("test" | "nextest" | "xtask" | "insta") => Self::Golden,
            _ => Self::Any,
        }
    }
}

impl Reuse {
    /// A cargo command that only answers a question (check, lint, test) may be served from an
    /// earlier pass; one that leaves something behind for the caller (build, run, install) never.
    pub(crate) fn of_cargo(args: &[String]) -> Self {
        match Need::subcommand(args) {
            Some("check" | "clippy" | "test" | "doc" | "deny") => Self::Whole,
            Some("nextest") if args.iter().any(|arg| arg == "run") => Self::Whole,
            _ => Self::Never,
        }
    }
}
