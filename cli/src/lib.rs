//! The `ink` command: a folder becomes a site.
//!
//! `ink build`, `ink check`, `ink explain` and `ink preview` (architecture.md section 14). Results
//! go to stdout, everything else to stderr; `--json` makes the result one JSON document with a
//! `schema` field and every diagnostic and notice an NDJSON event on stderr. Exit codes are the
//! closed set of research/flow.md section 7, [`Exit`].
//!
//! This crate is where the parts meet: `host-native` reads the folder, `config` reads `ink.toml`
//! and pins the engine's decisions, the markdown format reads pages, the engine interprets and
//! renders, and `ink:theme-quiet` writes the HTML.

mod args;
mod build;
mod error;
mod exit;
mod explain;
mod format;
mod out;
mod output;
mod pins;
mod preview;
mod report;
mod site;

pub use args::{Cli, Command};
pub use error::Error;
pub use exit::Exit;
pub use preview::Preview;

impl Cli {
    /// Runs the command, writing its result to `out` and everything else to `err`.
    pub fn run(self, out: &mut dyn std::io::Write, err: &mut dyn std::io::Write) -> Exit {
        let level = if self.verbose > 0 { output::Level::Notice } else { output::Level::Warning };
        let mut io = output::Io::new(out, err, self.json, level);
        let result = match self.command {
            Command::Build(build) => build.run(&mut io),
            Command::Check(check) => check.run(&mut io),
            Command::Explain(explain) => explain.run(&mut io),
            Command::Preview(preview) => preview.run(&mut io),
        };
        match result {
            Ok(exit) => exit,
            Err(error) => io.fail(&error),
        }
    }
}
