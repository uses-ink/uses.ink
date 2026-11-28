//! The `ink` command line as a stranger runs it: one binary, a home folder of its own so the keys
//! it keeps touch nothing of the person running the smoke.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::Fault;
use crate::process::{Failure, Process};

/// One `ink` binary with the home and API it runs with.
#[derive(Debug, Clone)]
pub(super) struct Cli {
    bin: PathBuf,
    home: PathBuf,
    api: Option<String>,
}

impl Cli {
    /// How many one-second tries a signed call gets while the server is quiet after its start.
    const TRIES: u32 = 45;

    /// The words the server uses for a start it has not yet taken signed requests after.
    const QUIET: &'static str = "auth.server.restarting";

    /// `bin`, keeping its keys under `home`.
    pub(super) fn new(bin: &Path, home: &Path) -> Self {
        Self { bin: bin.to_path_buf(), home: home.to_path_buf(), api: None }
    }

    /// The same, told which API to use through `INK_API`.
    pub(super) fn with_api(&self, api: &str) -> Self {
        Self { api: Some(api.to_owned()), ..self.clone() }
    }

    /// The binary.
    pub(super) fn bin(&self) -> &Path {
        &self.bin
    }

    /// Runs it in `dir` with `args` and returns its stdout. A call the server refuses for being
    /// quiet after its start is asked again, as the error says to.
    pub(super) fn run(&self, dir: &Path, args: &[&str]) -> Result<String, Fault> {
        let line = format!("ink {}", args.join(" "));
        for _ in 0..Self::TRIES {
            let mut command = Command::new(&self.bin);
            command
                .args(args)
                .current_dir(dir)
                .env("HOME", &self.home)
                .env("XDG_CONFIG_HOME", self.home.join(".config"))
                .env_remove("INK_KEY")
                .env_remove("INK_KEY_FILE");
            if let Some(api) = &self.api {
                command.env("INK_API", api);
            }
            match Process::from(command).text(&[]) {
                Ok(text) => return Ok(text),
                Err(Failure::Exit { stderr, .. }) if String::from_utf8_lossy(&stderr).contains(Self::QUIET) => {
                    std::thread::sleep(Duration::from_secs(1));
                }
                Err(source) => return Err(Fault::Command { line, source }),
            }
        }
        Err(Fault::Quiet { line })
    }
}
