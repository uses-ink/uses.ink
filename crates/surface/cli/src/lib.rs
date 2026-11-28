//! The `ink` command: a folder becomes a site.
//!
//! `ink build`, `ink check`, `ink explain`, `ink pin`, `ink preview`, `ink select`, `ink serve`,
//! `ink deploy`, `ink completions`, `ink man` and `ink help` (architecture.md section 14); the
//! commands of the tree are declared once in [`tree`](crate::grammar::tree). Results
//! go to stdout, everything else to stderr; `--json` makes the result one JSON document on stdout
//! with a `schema` field, an error's document (`uses.error/v1`) included, and every diagnostic,
//! and every stage that finishes, an NDJSON event on stderr; notices are printed only with `-v`,
//! in either form. Every kind of output is a typed block rendered once through one terminal layer
//! (decision 0061). Exit codes are the closed set of research/flow.md section 7, [`Exit`].
//!
//! This crate is where the parts meet: `host-native` reads the folder, `config` reads `ink.toml`
//! and pins the engine's decisions, the markdown format reads pages, the engine interprets and
//! renders, and `ink:theme-quiet` writes the HTML.

mod command;
mod deploy;
mod folder;
mod grammar;
mod logging;
mod output;
mod panic;
mod selfhost;
mod settings;

pub use command::preview::Preview;
pub use grammar::args::{Cli, Command, When};
pub use grammar::completions::Shell;
pub use output::error::Error;
pub use output::exit::Exit;
pub use output::term::Surroundings;
pub use panic::Panicked;
pub use settings::{Progress, Settings};

use grammar::{help, tree};
use output::failure::Failure;
use output::term::{self, Asked, Terminal};
use output::{Io, Mode, Say};

impl Cli {
    /// Parses `args` (the program's name first) and runs the command in a captured process: no
    /// terminal, no environment. See [`Cli::start_in`].
    ///
    /// # Errors
    ///
    /// The [`clap::Error`] of help or version, which clap prints.
    pub fn start<I, T>(args: I, out: &mut dyn std::io::Write, err: &mut dyn std::io::Write) -> Result<Exit, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::start_in(&Surroundings::empty(), args, out, err)
    }

    /// Parses `args` (the program's name first) and runs the command in `surroundings`. A command
    /// line clap refuses is reported as `error[cli.usage]`, in text on stderr or, under `--json`,
    /// as the one error document on stdout, and the exit is [`Exit::Usage`].
    ///
    /// # Errors
    ///
    /// The [`clap::Error`] of help and version, which clap prints, or of an error that could not
    /// be written.
    pub fn start_in<I, T>(
        surroundings: &Surroundings,
        args: I,
        out: &mut dyn std::io::Write,
        err: &mut dyn std::io::Write,
    ) -> Result<Exit, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let args: Vec<std::ffi::OsString> = args.into_iter().map(Into::into).collect();
        let json = args.iter().skip(1).any(|arg| arg == "--json");
        let command = tree::Tree::ink().command();
        let parsed = command.clone().try_get_matches_from(&args).and_then(|matches| {
            <Self as clap::FromArgMatches>::from_arg_matches(&matches)
                .map_err(|error| error.format(&mut command.clone()))
        });
        match parsed {
            Ok(cli) => Ok(cli.run_in(surroundings, out, err)),
            Err(error) => {
                let config = settings::Preferences::load(&settings::Process).ok().and_then(|file| file.ui.color);
                let terminal = Terminal::decide(surroundings, Asked { config, ..Asked::scan(&args) });
                match error.kind() {
                    clap::error::ErrorKind::DisplayHelp => {
                        Self::help(&args, help::Depth::of(&args), terminal.out, out).map(|()| Exit::Done)
                    }
                    clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
                        Self::help(&args, help::Depth::Short, terminal.err, err).map(|()| Exit::Usage)
                    }
                    _ if Self::refused(&error) => {
                        let mut io = Io::new(out, err, Mode { json, say: Say::Errors, terminal });
                        match io.failed(&Failure::usage(&error)) {
                            Ok(()) => Ok(Exit::Usage),
                            Err(_) => Err(error),
                        }
                    }
                    _ => Err(error),
                }
            }
        }
    }

    /// Writes the help of the command `args` name to `into`, in the faces of `look`.
    fn help(
        args: &[std::ffi::OsString],
        depth: help::Depth,
        look: term::Look,
        into: &mut dyn std::io::Write,
    ) -> Result<(), clap::Error> {
        let words: Vec<String> = args.iter().skip(1).filter_map(|arg| arg.to_str()).map(str::to_owned).collect();
        let tree = tree::Tree::ink();
        let text = help::Help::new(tree, look, help::Columns::of(&settings::Process))
            .render(&tree::Located::walk(tree, &words), depth);
        write!(into, "{text}").map_err(clap::Error::from)
    }

    /// Whether clap refused the command line, as it does for a mistyped one, and has not asked to
    /// print help.
    fn refused(error: &clap::Error) -> bool {
        error.use_stderr() && error.kind() != clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
    }

    /// Runs the command in a captured process, writing its result to `out` and everything else to
    /// `err`. See [`Cli::run_in`].
    pub fn run(self, out: &mut dyn std::io::Write, err: &mut dyn std::io::Write) -> Exit {
        self.run_in(&Surroundings::empty(), out, err)
    }

    /// Runs the command in `surroundings`, writing its result to `out` and everything else to
    /// `err`.
    pub fn run_in(
        self,
        surroundings: &Surroundings,
        out: &mut dyn std::io::Write,
        err: &mut dyn std::io::Write,
    ) -> Exit {
        let _logging = self.logged(err);
        let flagged = Asked { color: self.color, no_progress: self.no_progress, config: None };
        let settings = self.settings();
        let asked = settings.as_ref().map_or(flagged, |settings| Asked {
            color: Some(settings.color),
            no_progress: settings.progress == Progress::Plain,
            config: None,
        });
        let terminal = Terminal::decide(surroundings, asked);
        let mut io = Io::new(out, err, Mode { json: self.json, say: Say::of(self.quiet, self.verbose), terminal });
        if let Err(error) = settings {
            return io.fail(&error);
        }
        let site = self.dir.unwrap_or_else(|| std::path::PathBuf::from("."));
        match self.command.run(site, &mut io) {
            Ok(exit) => exit,
            Err(error) => io.fail(&error),
        }
    }
}
