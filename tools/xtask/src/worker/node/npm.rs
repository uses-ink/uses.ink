//! The npm packages of the Worker runs: `wrangler` and what it brings (`workerd`, `miniflare`),
//! pinned exactly in `spikes/hosted/package.json` and its lockfile, installed with `npm ci` under
//! `target/hosted/node/` so the farm's checkout stays source only.

use std::path::PathBuf;

use super::shell::Shell;
use crate::worker::fault::Fault;

/// The packages as the lockfile pins them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Packages<'a> {
    pub(crate) shell: &'a Shell,
}

impl Packages<'_> {
    /// Where the manifest and lockfile live, relative to the root.
    const SOURCE: &'static str = "spikes/hosted";

    /// The folder `npm ci` runs in.
    fn target(self) -> PathBuf {
        self.shell.root().join(Shell::HOME).join("node")
    }

    /// The lockfile that is committed.
    pub(crate) fn lockfile(self) -> PathBuf {
        self.shell.root().join(Self::SOURCE).join("package-lock.json")
    }

    /// Installs the packages unless the lockfile is the one already installed.
    pub(crate) fn install(self) -> Result<(), Fault> {
        let lock = Shell::read(&self.lockfile()).map_err(|_| Fault::Lock { path: self.lockfile() })?;
        let manifest = Shell::read(&self.shell.root().join(Self::SOURCE).join("package.json"))?;
        let target = self.target();
        let stamp = target.join("installed.stamp");
        let mut wanted = manifest.clone();
        wanted.extend_from_slice(&lock);
        if Shell::read(&stamp).is_ok_and(|held| held == wanted) && target.join("node_modules/.bin/wrangler").exists() {
            return Ok(());
        }
        Shell::write(&target.join("package.json"), &manifest)?;
        Shell::write(&target.join("package-lock.json"), &lock)?;
        let mut command = self.shell.command("npm", &target);
        command.arg("ci");
        Shell::run(command)?;
        Shell::write(&stamp, &wanted)
    }

    /// Resolves the manifest into a lockfile in a scratch folder and returns its text.
    pub(crate) fn resolve(self) -> Result<String, Fault> {
        let scratch = self.shell.root().join(Shell::HOME).join("lock");
        let manifest = Shell::read(&self.shell.root().join(Self::SOURCE).join("package.json"))?;
        Shell::write(&scratch.join("package.json"), &manifest)?;
        let mut command = self.shell.command("npm", &scratch);
        command.args(["install", "--package-lock-only", "--ignore-scripts"]);
        Shell::run(command)?;
        let path = scratch.join("package-lock.json");
        Ok(String::from_utf8_lossy(&Shell::read(&path)?).into_owned())
    }
}
