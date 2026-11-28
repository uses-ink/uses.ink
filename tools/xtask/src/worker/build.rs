//! Building a Worker crate for `wasm32-unknown-unknown` with `worker-build`, and measuring it.

use std::path::{Path, PathBuf};

use super::fault::Fault;
use super::shell::Shell;
use crate::process::Process;

/// How big a built Worker is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Size {
    /// The module's bytes.
    pub(super) raw: usize,
    /// The module's bytes after `gzip -9`.
    pub(super) gzip: usize,
}

/// A Worker crate and what building it takes.
#[derive(Debug, Clone)]
pub(super) struct Crate<'a> {
    pub(super) shell: &'a Shell,
    /// The crate's folder, relative to the root.
    pub(super) dir: &'static str,
}

impl Crate<'_> {
    /// The version of `worker-build` that matches `worker` 0.9.
    const BUILDER: &'static str = "=0.9.0";

    /// The crate's folder.
    pub(super) fn path(&self) -> PathBuf {
        self.shell.root().join(self.dir)
    }

    /// Installs `worker-build` under `target/hosted/tools` unless it is there.
    fn tool(&self) -> Result<(), Fault> {
        let root = self.shell.root().join(Shell::HOME).join("tools");
        if root.join("bin/worker-build").exists() {
            return Ok(());
        }
        let mut command = self.shell.command(env!("CARGO"), self.shell.root());
        command.args(["install", "worker-build", "--version", Self::BUILDER, "--locked", "--root"]).arg(&root);
        self.shell.run(command)
    }

    /// Builds the release module and returns its size.
    pub(super) fn build(&self) -> Result<Size, Fault> {
        self.tool()?;
        let mut command = self.shell.command("worker-build", &self.path());
        command.arg("--release");
        self.shell.run(command)?;
        self.size(&self.path().join("build/index_bg.wasm"))
    }

    /// The size of the file at `module`.
    fn size(&self, module: &Path) -> Result<Size, Fault> {
        let bytes = Shell::read(module)?;
        // gzip reads the file and writes the pipe: feeding its stdin from here while it fills its
        // stdout would deadlock once the compressed bytes pass the pipe's buffer.
        let mut gzip = self.shell.command("gzip", self.shell.root());
        gzip.args(["-9", "-c"]).arg(module);
        let zipped = Process::from(gzip)
            .stdout(&[])
            .map_err(|source| Fault::Command { program: "gzip -9 -c".to_owned(), source })?;
        Ok(Size { raw: bytes.len(), gzip: zipped.len() })
    }

    /// What `wrangler deploy --dry-run` says the upload is: its `Total Upload` line.
    pub(super) fn upload(&self) -> Result<String, Fault> {
        let out = self.shell.root().join(Shell::HOME).join("dry").join(self.dir.replace('/', "_"));
        let mut command = self.shell.command("wrangler", &self.path());
        command.args(["deploy", "--dry-run", "--outdir"]).arg(&out);
        let text = self.shell.text(command)?;
        Ok(text.lines().filter(|line| line.contains("Total Upload")).collect::<Vec<_>>().join("; "))
    }
}
