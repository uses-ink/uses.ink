//! The command line of `farm`: flags, then a verb and its arguments.
//!
//! The farm's flags come before the verb, or right after it; `cargo` and `bless` pass everything
//! after their own flags to cargo untouched (`--` ends the flags at once).

pub(crate) mod console;
pub(crate) mod digest;
pub(crate) mod error;
pub(crate) mod exit;
pub(crate) mod name;
pub(crate) mod version;

use std::fmt;
use std::num::ParseIntError;
use std::str::FromStr;
use std::time::Duration;

use crate::cli::error::Listed;
use crate::cli::name::{Fault, HostName, JobFault, JobId, JobName, Segment};

/// A verb, by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Check,
    Cargo,
    Bless,
    Fmt,
    Status,
    Workspace,
    Wait,
    Log,
}

impl Action {
    /// Every verb, in the order the docs list them.
    pub(crate) const ALL: [Self; 8] =
        [Self::Check, Self::Cargo, Self::Bless, Self::Fmt, Self::Status, Self::Workspace, Self::Wait, Self::Log];

    /// Its name on the command line.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Cargo => "cargo",
            Self::Bless => "bless",
            Self::Fmt => "fmt",
            Self::Status => "status",
            Self::Workspace => "workspace",
            Self::Wait => "wait",
            Self::Log => "log",
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

impl FromStr for Action {
    type Err = Usage;

    fn from_str(given: &str) -> Result<Self, Usage> {
        Self::ALL
            .into_iter()
            .find(|action| action.name() == given)
            .ok_or_else(|| Usage::Unknown { given: given.to_owned() })
    }
}

/// A number of seconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Seconds(pub(crate) u64);

impl Seconds {
    /// The hard limit of a job that names none.
    pub(crate) const LIMIT: Self = Self(1500);

    /// How long a call blocks by default: under the ten minutes a tool call may take.
    pub(crate) const TIMEOUT: Self = Self(540);

    pub(crate) const fn duration(self) -> Duration {
        Duration::from_secs(self.0)
    }
}

impl FromStr for Seconds {
    type Err = ParseIntError;

    fn from_str(given: &str) -> Result<Self, Self::Err> {
        given.parse().map(Self)
    }
}

/// Everything a flag can set.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "each is a switch of the command line")]
pub(crate) struct Flags {
    /// `--json`: print the machine report.
    pub(crate) json: bool,
    /// `--verbose`: print every command run.
    pub(crate) verbose: bool,
    /// `--detach`: start the job and print its id.
    pub(crate) detach: bool,
    /// `--timeout`: how long a call blocks; 540 s, under the 10 minutes a tool call may take.
    pub(crate) timeout: Seconds,
    /// `--limit`: the hard limit of a job's command; else the job's own, else 1500 s.
    pub(crate) limit: Option<Seconds>,
    /// `--host`: the only host to use.
    pub(crate) host: Option<HostName>,
    /// `--tail`: the last lines of a log.
    pub(crate) tail: Option<u32>,
    /// `--at`: the revision a new workspace starts from.
    pub(crate) at: Option<String>,
    /// `--forget`: remove a workspace.
    pub(crate) forget: bool,
    /// `--fresh`: run the job although an identical one passed before.
    pub(crate) fresh: bool,
    /// `--budget`: warn after this many full checks in a day; else the job file's.
    pub(crate) budget: Option<u32>,
}

impl Default for Flags {
    fn default() -> Self {
        Self {
            json: false,
            verbose: false,
            detach: false,
            timeout: Seconds::TIMEOUT,
            limit: None,
            host: None,
            tail: None,
            at: None,
            forget: false,
            fresh: false,
            budget: None,
        }
    }
}

impl Flags {
    /// Takes `arg` if it is one of ours, with its value from `rest`; false if it is not a flag.
    fn take(&mut self, arg: &str, rest: &mut impl Iterator<Item = String>) -> Result<bool, Usage> {
        let mut value = |flag: &str| rest.next().ok_or_else(|| Usage::Value { flag: flag.to_owned() });
        let bad = |flag: &str, why| Usage::Flag { flag: flag.to_owned(), why };
        match arg {
            "--json" => self.json = true,
            "--verbose" => self.verbose = true,
            "--detach" => self.detach = true,
            "--forget" => self.forget = true,
            "--fresh" => self.fresh = true,
            "--budget" => self.budget = Some(value(arg)?.parse().map_err(|e| bad(arg, Why::Number(e)))?),
            "--timeout" => self.timeout = value(arg)?.parse().map_err(|e| bad(arg, Why::Number(e)))?,
            "--limit" => self.limit = Some(value(arg)?.parse().map_err(|e| bad(arg, Why::Number(e)))?),
            "--tail" => self.tail = Some(value(arg)?.parse().map_err(|e| bad(arg, Why::Number(e)))?),
            "--host" => self.host = Some(value(arg)?.parse().map_err(|e| bad(arg, Why::Word(e)))?),
            "--at" => {
                let rev = value(arg)?;
                if rev.starts_with('-') || rev.is_empty() {
                    return Err(bad(arg, Why::Rev { given: rev }));
                }
                self.at = Some(rev);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// A verb with its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Verb {
    /// The repo's jobs, fanned out: these, or all.
    Check(Vec<JobName>),
    /// A cargo command, as given.
    Cargo(Vec<String>),
    /// Nextest with its snapshots accepted; the arguments are nextest's.
    Bless(Vec<String>),
    /// `cargo fmt --all` on a host, the changed `.rs` files brought back.
    Fmt,
    Status,
    /// A new workspace of this name.
    Make(Segment),
    /// Remove the workspace of this name.
    Forget(Segment),
    Wait(JobId),
    Log(JobId),
    /// `--version`: what the binary was built from.
    Version,
}

/// A parsed command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Command {
    pub(crate) flags: Flags,
    pub(crate) verb: Verb,
}

impl Command {
    /// Reads the arguments after `farm`.
    pub(crate) fn parse(args: Vec<String>) -> Result<Self, Usage> {
        let mut flags = Flags::default();
        let mut rest = args.into_iter().peekable();
        let action = loop {
            let arg = rest.next().ok_or(Usage::NoVerb)?;
            if arg == "--version" {
                return Ok(Self { flags, verb: Verb::Version });
            }
            if !flags.take(&arg, &mut rest)? {
                break arg.parse::<Action>()?;
            }
        };
        let verb = match action {
            Action::Cargo | Action::Bless => {
                let mut tail = Vec::new();
                while let Some(arg) = rest.next() {
                    if arg == "--" {
                        // the first `--` only ends the farm's flags; any later one is cargo's
                        if !tail.is_empty() {
                            tail.push(arg);
                        }
                        tail.extend(rest.by_ref());
                    } else if !(tail.is_empty() && flags.take(&arg, &mut rest)?) {
                        tail.push(arg);
                    }
                }
                if action == Action::Cargo && tail.is_empty() {
                    return Err(Usage::Missing { verb: action, what: "the cargo arguments" });
                }
                if action == Action::Cargo { Verb::Cargo(tail) } else { Verb::Bless(tail) }
            }
            _ => Self::plain(action, &mut flags, rest)?,
        };
        Ok(Self { flags, verb })
    }

    /// The verbs whose arguments are positional words, with flags anywhere.
    fn plain(action: Action, flags: &mut Flags, mut rest: impl Iterator<Item = String>) -> Result<Verb, Usage> {
        let mut words = Vec::new();
        while let Some(arg) = rest.next() {
            if !flags.take(&arg, &mut rest)? {
                words.push(arg);
            }
        }
        let mut words = words.into_iter();
        let verb = match action {
            Action::Check => {
                let names = words.by_ref().map(|given| given.parse().map_err(Usage::Task)).collect::<Result<_, _>>()?;
                Verb::Check(names)
            }
            Action::Status => Verb::Status,
            Action::Fmt => Verb::Fmt,
            Action::Workspace => {
                let name = words.next().ok_or(Usage::Missing { verb: action, what: "a workspace name" })?;
                let name: Segment = name.parse().map_err(Usage::Name)?;
                if flags.forget { Verb::Forget(name) } else { Verb::Make(name) }
            }
            Action::Wait | Action::Log => {
                let id = words.next().ok_or(Usage::Missing { verb: action, what: "a job id" })?;
                let id = id.parse().map_err(Usage::Job)?;
                if action == Action::Wait { Verb::Wait(id) } else { Verb::Log(id) }
            }
            Action::Cargo | Action::Bless => return Err(Usage::NoVerb),
        };
        match words.next() {
            Some(given) => Err(Usage::Extra { verb: action, given }),
            None => Ok(verb),
        }
    }
}

/// A command line that is wrong.
#[derive(Debug)]
pub(crate) enum Usage {
    NoVerb,
    Unknown { given: String },
    Missing { verb: Action, what: &'static str },
    Extra { verb: Action, given: String },
    Task(Fault),
    Value { flag: String },
    Flag { flag: String, why: Why },
    Name(Fault),
    Job(JobFault),
}

/// Why a flag's value is not one.
#[derive(Debug)]
pub(crate) enum Why {
    Number(ParseIntError),
    Word(Fault),
    Rev { given: String },
}

impl fmt::Display for Usage {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoVerb => write!(out, "no verb given; the verbs are {}", Listed(&Action::ALL)),
            Self::Unknown { given } => write!(out, "no verb `{given}`; the verbs are {}", Listed(&Action::ALL)),
            Self::Missing { verb, what } => write!(out, "`farm {verb}` needs {what}"),
            Self::Extra { verb, given } => write!(out, "`farm {verb}` takes no `{given}`"),
            Self::Task(_) => write!(out, "that is not a job name"),
            Self::Value { flag } => write!(out, "`{flag}` needs a value"),
            Self::Flag { flag, why } => match why {
                Why::Number(_) => write!(out, "`{flag}` takes a whole number"),
                Why::Word(_) => write!(out, "`{flag}` takes a host name"),
                Why::Rev { given } => write!(out, "`{flag}` takes a revision, not `{given}`"),
            },
            Self::Name(_) => write!(out, "the workspace name is not allowed"),
            Self::Job(_) => write!(out, "that is not a job id"),
        }
    }
}

impl std::error::Error for Usage {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Flag { why: Why::Number(source), .. } => Some(source),
            Self::Flag { why: Why::Word(source), .. } | Self::Name(source) | Self::Task(source) => Some(source),
            Self::Job(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Result<Command, Usage> {
        Command::parse(line.split_whitespace().map(str::to_owned).collect())
    }

    fn words(line: &str) -> Vec<String> {
        line.split_whitespace().map(str::to_owned).collect()
    }

    #[test]
    fn a_double_dash_after_the_subcommand_and_what_follows_reach_cargo_verbatim() {
        let line = "clippy -p x --all-targets -- -D warnings";
        assert_eq!(parse(&format!("cargo {line}")).unwrap().verb, Verb::Cargo(words(line)));
        let more = "nextest run -p x -- --nocapture --json";
        assert_eq!(parse(&format!("--detach cargo {more}")).unwrap().verb, Verb::Cargo(words(more)));
        let flags = parse("cargo clippy -- --json -D warnings").unwrap();
        assert!(!flags.flags.json, "after the subcommand `--json` is cargo's, `--` or not");
        assert_eq!(parse("bless -p x -- name").unwrap().verb, Verb::Bless(words("-p x -- name")));
    }

    #[test]
    fn a_check_names_jobs_or_none() {
        assert_eq!(parse("check").unwrap().verb, Verb::Check(vec![]));
        assert_eq!(
            parse("check lint test").unwrap().verb,
            Verb::Check(vec!["lint".parse().unwrap(), "test".parse().unwrap()])
        );
        assert!(matches!(parse("check ../x"), Err(Usage::Task(_))));
    }

    #[test]
    fn cargo_arguments_pass_through_and_farm_flags_stop_at_the_first_other_word() {
        let command = parse("--json cargo --detach clippy -p engine --verbose").unwrap();
        assert!(command.flags.json && command.flags.detach);
        assert!(!command.flags.verbose, "after the cargo arguments start, `--verbose` is cargo's");
        assert_eq!(command.verb, Verb::Cargo(["clippy", "-p", "engine", "--verbose"].map(str::to_owned).to_vec()));
        let command = parse("cargo -- --json nextest").unwrap();
        assert!(!command.flags.json);
        assert_eq!(command.verb, Verb::Cargo(["--json", "nextest"].map(str::to_owned).to_vec()));
        let command = parse("cargo -- clippy -p x -- -D warnings").unwrap();
        assert_eq!(command.verb, Verb::Cargo(words("clippy -p x -- -D warnings")), "a leading `--` is the farm's");
        assert!(matches!(parse("cargo"), Err(Usage::Missing { .. })));
        assert!(matches!(parse("cargo --json"), Err(Usage::Missing { .. })));
    }

    #[test]
    fn fresh_and_budget_are_farm_flags_before_or_after_the_verb_but_not_after_cargo_words() {
        let flags = parse("--fresh check --budget 3 lint").unwrap().flags;
        assert!(flags.fresh);
        assert_eq!(flags.budget, Some(3));
        assert!(parse("cargo --fresh nextest run").unwrap().flags.fresh);
        assert!(!parse("cargo nextest run --fresh").unwrap().flags.fresh, "after cargo's words it is cargo's");
        assert!(parse("check --budget x").is_err());
        assert!(!parse("check").unwrap().flags.fresh);
    }

    #[test]
    fn fmt_takes_nothing() {
        assert_eq!(parse("fmt").unwrap().verb, Verb::Fmt);
        assert!(matches!(parse("fmt -p cli"), Err(Usage::Extra { .. })));
    }

    #[test]
    fn bless_takes_nextest_arguments_or_none() {
        assert_eq!(parse("bless").unwrap().verb, Verb::Bless(vec![]));
        assert_eq!(parse("bless -p cli").unwrap().verb, Verb::Bless(vec!["-p".to_owned(), "cli".to_owned()]));
    }

    #[test]
    fn wait_and_log_take_a_job_id_and_their_flags() {
        let command = parse("wait petal.1a2 --timeout 30").unwrap();
        assert_eq!(command.flags.timeout, Seconds(30));
        assert!(matches!(command.verb, Verb::Wait(_)));
        assert_eq!(parse("log petal.1a2 --tail 20").unwrap().flags.tail, Some(20));
        assert!(matches!(parse("wait"), Err(Usage::Missing { .. })));
        assert!(matches!(parse("wait nope"), Err(Usage::Job(_))));
        assert!(matches!(parse("wait a.1 b.2"), Err(Usage::Extra { .. })));
    }

    #[test]
    fn workspace_makes_or_forgets() {
        assert!(matches!(parse("workspace x --at main").unwrap().verb, Verb::Make(_)));
        assert!(matches!(parse("workspace --forget x").unwrap().verb, Verb::Forget(_)));
        assert!(matches!(parse("workspace ../x"), Err(Usage::Name(_))));
        assert!(matches!(parse("workspace x --at --evil"), Err(Usage::Flag { .. })));
    }

    #[test]
    fn defaults_are_the_documented_ones_and_bad_input_is_usage() {
        let flags = parse("status").unwrap().flags;
        assert_eq!((flags.timeout, flags.limit), (Seconds::TIMEOUT, None));
        for bad in ["", "frob", "status --timeout", "status --timeout x", "status --host A b", "status extra"] {
            assert!(parse(bad).is_err(), "{bad:?}");
        }
    }
}
