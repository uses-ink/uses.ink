//! `ink`: see the crate documentation of `uses-ink-cli`.

use std::process::ExitCode;

use uses_ink_cli::Cli;

fn main() -> ExitCode {
    match Cli::start(std::env::args_os(), &mut std::io::stdout().lock(), &mut std::io::stderr().lock()) {
        Ok(exit) => ExitCode::from(exit.code()),
        Err(error) => error.exit(),
    }
}
