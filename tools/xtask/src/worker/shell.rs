//! The commands a Worker run starts: node and npm from the host's `~/.local/bin`, the tools this
//! run installs under `target/hosted/`, and a few settings that keep wrangler quiet and offline.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::fault::Fault;
use crate::process::Process;

/// Where the commands of one run start from and find their tools.
#[derive(Debug, Clone)]
pub(super) struct Shell {
    root: PathBuf,
    path: OsString,
}

impl Shell {
    /// Where this run keeps what it installs and builds, relative to the root.
    pub(super) const HOME: &'static str = "target/hosted";

    /// A shell for the workspace at `root`: the tools of the run first, then the host's own.
    pub(super) fn new(root: &Path) -> Self {
        let home = root.join(Self::HOME);
        let mut dirs = vec![home.join("tools/bin"), home.join("node/node_modules/.bin")];
        if let Some(user) = std::env::var_os("HOME") {
            dirs.push(Path::new(&user).join(".local/bin"));
        }
        dirs.extend(std::env::var_os("PATH").iter().flat_map(std::env::split_paths));
        let path = std::env::join_paths(dirs).unwrap_or_default();
        Self { root: root.to_path_buf(), path }
    }

    /// The workspace root.
    pub(super) fn root(&self) -> &Path {
        &self.root
    }

    /// `program` run in `dir`, with this shell's `PATH` and no prompts, telemetry or update checks.
    pub(super) fn command(&self, program: &str, dir: &Path) -> Command {
        let mut command = Command::new(program);
        command
            .current_dir(dir)
            .env("PATH", &self.path)
            .env("CI", "1")
            .env("WRANGLER_SEND_METRICS", "false")
            .env("NO_UPDATE_NOTIFIER", "1")
            .env("NPM_CONFIG_UPDATE_NOTIFIER", "false")
            .env("NPM_CONFIG_FUND", "false")
            .env("NPM_CONFIG_AUDIT", "false");
        command
    }

    /// `command` run to its end, its output going where the farm's log is.
    pub(super) fn run(&self, command: Command) -> Result<(), Fault> {
        let program = Self::name(&command);
        Process::from(command).status().map_err(|source| Fault::Command { program, source })
    }

    /// What `command` printed, as text.
    pub(super) fn text(&self, command: Command) -> Result<String, Fault> {
        let program = Self::name(&command);
        Process::from(command).text(&[]).map_err(|source| Fault::Command { program, source })
    }

    /// The command line, for saying what failed.
    pub(super) fn name(command: &Command) -> String {
        std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|word| word.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// A file's bytes.
    pub(super) fn read(path: &Path) -> Result<Vec<u8>, Fault> {
        std::fs::read(path).map_err(|source| Fault::File { path: path.to_path_buf(), source })
    }

    /// Writes a file, and the folders above it.
    pub(super) fn write(path: &Path, bytes: &[u8]) -> Result<(), Fault> {
        let file = |source| Fault::File { path: path.to_path_buf(), source };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(file)?;
        }
        std::fs::write(path, bytes).map_err(file)
    }
}
