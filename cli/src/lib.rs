//! The `ink` command: a folder becomes a site.
//!
//! `ink build`, `ink check`, `ink explain`, `ink pin`, `ink preview`, `ink select` and `ink serve` (architecture.md section 14). Results
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
mod component;
mod error;
mod exit;
mod explain;
mod format;
mod out;
mod output;
mod pin;
mod pins;
mod preview;
mod product;
mod report;
mod select;
mod serve;
mod site;

pub use args::{Cli, Command};
pub use error::Error;
pub use exit::Exit;
pub use preview::Preview;

impl Cli {
    /// Parses `args` (the program's name first) and runs the command. A command line clap
    /// refuses is returned to the caller to report, except under `--json`, where it is the one
    /// error document on stdout and the exit is [`Exit::Usage`].
    ///
    /// # Errors
    ///
    /// The [`clap::Error`] of a command line that does not parse, help and version among them,
    /// when `--json` was not given or clap prints it to stdout.
    pub fn start<I, T>(args: I, out: &mut dyn std::io::Write, err: &mut dyn std::io::Write) -> Result<Exit, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let args: Vec<std::ffi::OsString> = args.into_iter().map(Into::into).collect();
        let json = args.iter().skip(1).any(|arg| arg == "--json");
        match <Self as clap::Parser>::try_parse_from(&args) {
            Ok(cli) => Ok(cli.run(out, err)),
            Err(error) if json && error.use_stderr() => {
                let mut io = output::Io::new(out, err, true, output::Level::Warning);
                match io.json(&output::Failed::usage(&error)) {
                    Ok(()) => Ok(Exit::Usage),
                    Err(_) => Err(error),
                }
            }
            Err(error) => Err(error),
        }
    }

    /// Runs the command, writing its result to `out` and everything else to `err`.
    pub fn run(self, out: &mut dyn std::io::Write, err: &mut dyn std::io::Write) -> Exit {
        let level = if self.verbose > 0 { output::Level::Notice } else { output::Level::Warning };
        let mut io = output::Io::new(out, err, self.json, level);
        let result = match self.command {
            Command::Build(build) => build.run(&mut io),
            Command::Check(check) => check.run(&mut io),
            Command::Explain(explain) => explain.run(&mut io),
            Command::Pin(pin) => pin.run(&mut io),
            Command::Preview(preview) => preview.run(&mut io),
            Command::Select(select) => select.run(&mut io),
            Command::Serve(serve) => serve.run(&mut io),
        };
        match result {
            Ok(exit) => exit,
            Err(error) => io.fail(&error),
        }
    }
}
