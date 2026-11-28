//! The seam to logging (decision 0065): the two flags and the one call that installs it. Nothing
//! else in the CLI configures logging, and what it writes is on stderr or in a file, never on
//! stdout and never inside a `--json` document.

use std::io::Write;

use logging::{Entered, Guard, Logging, Process, Variables, Vars};
use model::Chain;

use crate::settings::{Log, Preferences, Process as Terminal};
use crate::{Cli, Command};

/// What a run keeps for logging: the command's span, then the guard that prints the timing table
/// when the run ends. Field order is drop order.
#[derive(Debug)]
pub(crate) struct Scope {
    _span: Entered,
    _guard: Guard,
}

impl Command {
    /// The verb, as the `command` span names it.
    const fn verb(&self) -> &'static str {
        match self {
            Self::Init(_) => "init",
            Self::Check(_) => "check",
            Self::Build(_) => "build",
            Self::Preview(_) => "preview",
            Self::Serve(_) => "serve",
            Self::Explain(_) => "explain",
            Self::Pin(_) => "pin",
            Self::Select(_) => "select",
            Self::Login(_) => "login",
            Self::Logout(_) => "logout",
            Self::Whoami(_) => "whoami",
            Self::Key(_) => "key",
            Self::Abuse(_) => "abuse",
            Self::Push(_) => "push",
            Self::Status(_) => "status",
            Self::Watch(_) => "watch",
            Self::Deploy(_) | Self::Deploys(_) => "deploy",
            Self::Export(_) => "export",
            Self::Completions(_) => "completions",
            Self::Man => "man",
            Self::Help(_) => "help",
        }
    }
}

/// The process environment with the config file's `[log]` table under it: a variable wins over
/// the file, which is the one precedence of `settings.rs` (flag, variable, file, default).
struct Layered<'a> {
    file: &'a Log,
}

impl Vars for Layered<'_> {
    fn var(&self, name: &str) -> Option<String> {
        let held = match name {
            Variables::FILTER => self.file.filter.clone(),
            Variables::FILE => self.file.file.as_ref().map(|path| path.display().to_string()),
            Variables::FORMAT => self.file.format.clone(),
            _ => None,
        };
        Process.var(name).or(held)
    }
}

impl Cli {
    /// The logging the settings ask for, weakest first: the `-v` count, the config file, the
    /// variables, then the flags (which clap also reads `INK_LOG` and `INK_LOG_FILE` into).
    fn logging(&self) -> Result<Logging, logging::Error> {
        // a config file that cannot be read is reported by `settings`, not here
        let preferences = Preferences::load(&Terminal).unwrap_or_default();
        let mut logging =
            Logging::new().env(&Layered { file: &preferences.log })?.verbosity(self.verbose).machine(self.json);
        if let Some(filter) = &self.log {
            logging = logging.filter(filter.clone());
        }
        if let Some(path) = &self.log_file {
            logging = logging.file(path);
        }
        Ok(logging)
    }

    /// Sets logging up, and opens the command's span. A failure is said on `err` as text and the
    /// run goes on without logging; under `--json` stderr is a machine channel, so it is not said
    /// there.
    pub(crate) fn logged(&self, err: &mut dyn Write) -> Option<Scope> {
        let guard = match self.logging().and_then(Logging::install) {
            Ok(guard) => guard,
            Err(error) => {
                if !self.json {
                    // the stream has nowhere to report its own failure
                    writeln!(err, "warning: logging is off: {error}").ok();
                    for cause in Chain::new(&error).causes() {
                        writeln!(err, "  caused by: {cause}").ok();
                    }
                }
                return None;
            }
        };
        Some(Scope { _span: logging::command!(self.command.verb()).entered(), _guard: guard })
    }
}
