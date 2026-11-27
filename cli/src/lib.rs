//! The `ink` command: a folder becomes a site.
//!
//! `ink build`, `ink check`, `ink explain`, `ink pin`, `ink preview`, `ink select`, `ink serve`,
//! `ink deploy`, `ink completions`, `ink man` and `ink help` (architecture.md section 14); the
//! commands of the tree are declared once in [`tree`](crate::tree). Results
//! go to stdout, everything else to stderr; `--json` makes the result one JSON document on stdout
//! with a `schema` field, an error's document (`uses.error/v1`) included, and every diagnostic an
//! NDJSON event on stderr; notices are printed only with `-v`, in either form. Exit codes are the
//! closed set of research/flow.md section 7, [`Exit`].
//!
//! This crate is where the parts meet: `host-native` reads the folder, `config` reads `ink.toml`
//! and pins the engine's decisions, the markdown format reads pages, the engine interprets and
//! renders, and `ink:theme-quiet` writes the HTML.

mod args;
mod asked;
mod build;
mod bundle;
mod cache;
mod change;
mod completions;
mod component;
mod deploy;
mod dispatch;
mod error;
mod exit;
mod explain;
mod format;
mod help;
mod man;
mod out;
mod output;
mod pin;
mod pins;
mod preview;
mod product;
mod replace;
mod report;
mod select;
mod serve;
mod settings;
mod site;
mod tree;

pub use args::{Cli, Command, When};
pub use completions::Shell;
pub use error::Error;
pub use exit::Exit;
pub use preview::Preview;
pub use settings::{Progress, Settings};

impl Cli {
    /// Parses `args` (the program's name first) and runs the command. Help is written by
    /// [`help::Help`] from the command tree: to `out` when it was asked for, to `err` with
    /// [`Exit::Usage`] when a command was missing. A command line clap refuses is returned to the
    /// caller to report, except under `--json`, where it is the one error document on stdout and
    /// the exit is [`Exit::Usage`].
    ///
    /// # Errors
    ///
    /// The [`clap::Error`] of a command line that does not parse, the version among them, when
    /// `--json` was not given or clap prints it to stdout.
    pub fn start<I, T>(args: I, out: &mut dyn std::io::Write, err: &mut dyn std::io::Write) -> Result<Exit, clap::Error>
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
            Ok(cli) => Ok(cli.run(out, err)),
            Err(error) => match error.kind() {
                clap::error::ErrorKind::DisplayHelp => {
                    Self::help(&args, help::Depth::of(&args), out).map(|()| Exit::Done)
                }
                clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
                    Self::help(&args, help::Depth::Short, err).map(|()| Exit::Usage)
                }
                _ if json && error.use_stderr() => {
                    let mut io = output::Io::new(out, err, true, output::Level::Warning);
                    match io.json(&output::Failed::usage(&error)) {
                        Ok(()) => Ok(Exit::Usage),
                        Err(_) => Err(error),
                    }
                }
                _ => Err(error),
            },
        }
    }

    /// Writes the help of the command `args` name to `into`.
    fn help(args: &[std::ffi::OsString], depth: help::Depth, into: &mut dyn std::io::Write) -> Result<(), clap::Error> {
        let words: Vec<String> = args.iter().skip(1).filter_map(|arg| arg.to_str()).map(str::to_owned).collect();
        let tree = tree::Tree::ink();
        let painter = help::Painter::declared(args, &settings::Process);
        let text = help::Help::new(tree, painter, help::Columns::of(&settings::Process))
            .render(&tree::Located::walk(tree, &words), depth);
        write!(into, "{text}").map_err(clap::Error::from)
    }

    /// Runs the command, writing its result to `out` and everything else to `err`.
    pub fn run(self, out: &mut dyn std::io::Write, err: &mut dyn std::io::Write) -> Exit {
        let level = if self.verbose > 0 { output::Level::Notice } else { output::Level::Warning };
        let mut io = output::Io::new(out, err, self.json, level);
        let settings = match self.settings() {
            Ok(settings) => settings,
            Err(error) => return io.fail(&error),
        };
        let site = self.dir.unwrap_or_else(|| std::path::PathBuf::from("."));
        match self.command.run(site, settings, &mut io) {
            Ok(exit) => exit,
            Err(error) => io.fail(&error),
        }
    }
}
