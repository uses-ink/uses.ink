//! The host side of the browser: node and npm from the host's `~/.local/bin`, the pinned packages of
//! `tools/shoot/` installed with `npm ci` under `target/shoot/node/`, the browser Playwright downloads
//! into the host's cache, and the driver run over a plan.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::fault::Fault;
use super::plan::Plan;
use crate::process::Process;

/// Where the commands of one run start from and find their tools.
#[derive(Debug, Clone)]
pub(crate) struct Host {
    root: PathBuf,
    path: OsString,
    browsers: Option<PathBuf>,
}

impl Host {
    /// Where this run keeps what it installs and builds, relative to the root.
    pub(crate) const HOME: &'static str = "target/shoot";

    /// The packages' manifest, lockfile and driver, relative to the root.
    const SOURCE: &'static str = "tools/shoot";

    /// A host for the workspace at `root`: node from `~/.local/bin` first, then the host's own `PATH`.
    pub(crate) fn new(root: &Path) -> Self {
        let user = std::env::var_os("HOME").map(PathBuf::from);
        let mut dirs = vec![root.join(Self::HOME).join("node/node_modules/.bin")];
        dirs.extend(user.iter().map(|home| home.join(".local/bin")));
        dirs.extend(std::env::var_os("PATH").iter().flat_map(std::env::split_paths));
        Self {
            root: root.to_path_buf(),
            path: std::env::join_paths(dirs).unwrap_or_default(),
            browsers: user.map(|home| home.join(".cache/uses-ink/browsers")),
        }
    }

    /// `program` run in `dir`, with this host's `PATH`, the browser cache and no prompts or telemetry.
    pub(crate) fn command(&self, program: &str, dir: &Path) -> Command {
        let mut command = Command::new(program);
        command
            .current_dir(dir)
            .env("PATH", &self.path)
            .env("CI", "1")
            .env("NO_UPDATE_NOTIFIER", "1")
            .env("NPM_CONFIG_UPDATE_NOTIFIER", "false")
            .env("NPM_CONFIG_FUND", "false")
            .env("NPM_CONFIG_AUDIT", "false");
        if let Some(browsers) = &self.browsers {
            command.env("PLAYWRIGHT_BROWSERS_PATH", browsers);
        }
        command
    }

    /// `command` run to its end, its output going where the farm's log is.
    pub(crate) fn run(command: Command) -> Result<(), Fault> {
        let line = std::iter::once(command.get_program())
            .chain(command.get_args())
            .map(|word| word.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        Process::from(command).status().map_err(|source| Fault::Command { line, source })
    }

    /// The folder `npm ci` runs in.
    fn target(&self) -> PathBuf {
        self.root.join(Self::HOME).join("node")
    }

    /// A file's bytes.
    fn read(path: &Path) -> Result<Vec<u8>, Fault> {
        std::fs::read(path).map_err(|source| Fault::File { path: path.to_path_buf(), source })
    }

    /// Writes a file, and the folders above it.
    fn write(path: &Path, bytes: &[u8]) -> Result<(), Fault> {
        let file = |source| Fault::File { path: path.to_path_buf(), source };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(file)?;
        }
        std::fs::write(path, bytes).map_err(file)
    }

    /// The lockfile that is committed.
    pub(crate) fn lockfile(&self) -> PathBuf {
        self.root.join(Self::SOURCE).join("package-lock.json")
    }

    /// Resolves the manifest into a lockfile in a scratch folder and returns its text.
    pub(crate) fn resolve(&self) -> Result<String, Fault> {
        let scratch = self.root.join(Self::HOME).join("lock");
        Self::write(&scratch.join("package.json"), &Self::read(&self.root.join(Self::SOURCE).join("package.json"))?)?;
        let mut command = self.command("npm", &scratch);
        command.args(["install", "--package-lock-only", "--ignore-scripts"]);
        Self::run(command)?;
        Ok(String::from_utf8_lossy(&Self::read(&scratch.join("package-lock.json"))?).into_owned())
    }

    /// Installs the packages unless the lockfile is the one already installed, and copies the driver.
    pub(crate) fn install(&self) -> Result<(), Fault> {
        let source = self.root.join(Self::SOURCE);
        let lock = Self::read(&self.lockfile()).map_err(|_| Fault::Lock { path: self.lockfile() })?;
        let manifest = Self::read(&source.join("package.json"))?;
        let target = self.target();
        Self::write(&target.join("shoot.mjs"), &Self::read(&source.join("shoot.mjs"))?)?;
        let stamp = target.join("installed.stamp");
        let mut wanted = manifest.clone();
        wanted.extend_from_slice(&lock);
        if Self::read(&stamp).is_ok_and(|held| held == wanted) && target.join("node_modules/.bin").exists() {
            return Ok(());
        }
        Self::write(&target.join("package.json"), &manifest)?;
        Self::write(&target.join("package-lock.json"), &lock)?;
        let mut command = self.command("npm", &target);
        command.arg("ci");
        Self::run(command)?;
        Self::write(&stamp, &wanted)
    }

    /// Downloads Playwright's headless Chromium into the host's cache. A download that fails is
    /// returned, not fatal: the driver tries the static build of `@sparticuz/chromium` next and says
    /// why neither started if neither does.
    pub(crate) fn fetch(&self) -> Result<(), Fault> {
        let mut command = self.command("node", &self.target());
        command.args(["node_modules/playwright-core/cli.js", "install", "chromium-headless-shell"]);
        Self::run(command)
    }

    /// Runs the driver over `plan`, its pictures printed into the log and written under `plan.out`.
    pub(crate) fn drive(&self, plan: &Plan) -> Result<(), Fault> {
        let file = self.root.join(Self::HOME).join("plan.json");
        plan.write(&file)?;
        let mut command = self.command("node", &self.target());
        command.arg("shoot.mjs").arg(file);
        Self::run(command)
    }
}
