//! `ink`: see the crate documentation of `uses-ink-cli`.

use std::process::ExitCode;

use clap::Parser;
use uses_ink_cli::Cli;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let exit = cli.run(&mut std::io::stdout().lock(), &mut std::io::stderr().lock());
    ExitCode::from(exit.code())
}
