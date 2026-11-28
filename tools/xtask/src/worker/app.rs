//! The product Worker (decision 0201): its migrations written as wrangler's files, and the whole
//! Worker built, started in workerd with a local D1, R2, Queues and Durable Object, and driven by
//! the real `ink` and a raw signed client (`crates/surface/cli/tests/hosted.rs`).

use std::path::PathBuf;

use super::case::Case;
use super::fault::Fault;
use super::node::{Crate, Packages, Shell};
use crate::report::{Line, Report};

/// One run.
#[derive(Debug, Clone, Copy)]
pub(super) struct Hosted<'a> {
    pub(super) shell: &'a Shell,
}

impl Hosted<'_> {
    /// The Worker's folder, relative to the root.
    pub(super) const DIR: &'static str = "crates/server/edge/app";

    /// The folder this run works in, under `target/`: the migrations, the local state, the report.
    fn work(self) -> PathBuf {
        self.shell.root().join(Shell::HOME).join("app")
    }

    /// Writes the migrations to `<work>/migrations`, which the Worker's `wrangler.toml` points at.
    pub(super) fn migrations(self, report: &mut Report) -> Result<(), Fault> {
        let work = self.work();
        let mut write = self.shell.command(env!("CARGO"), self.shell.root());
        write
            .args(["run", "--quiet", "-p", "uses-ink-edge-worker", "--features", "scripts", "--bin", "scripts", "--"])
            .arg(&work);
        Shell::run(write)?;
        let files = std::fs::read_dir(work.join("migrations"))
            .map_err(|source| Fault::File { path: work.join("migrations"), source })?
            .count();
        report.line(Line::Worker {
            case: Case::Migrations,
            text: format!("wrote {files} migrations to {}", work.join("migrations").display()),
        });
        Ok(())
    }

    /// Builds, migrates, starts, and drives the Worker.
    pub(super) fn run(self, report: &mut Report) -> Result<(), Fault> {
        Packages { shell: self.shell }.install()?;
        let work = self.work();
        match std::fs::remove_dir_all(&work) {
            Ok(()) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(Fault::File { path: work, source }),
        }
        self.migrations(report)?;
        let krate = Crate { shell: self.shell, dir: Self::DIR };
        let started = std::time::Instant::now();
        let size = krate.build()?;
        let mut say = |text: String| report.line(Line::Worker { case: Case::App, text });
        say(format!(
            "built in {} s: index_bg.wasm {} bytes raw, {} bytes gzip -9",
            started.elapsed().as_secs(),
            size.raw,
            size.gzip
        ));
        say(format!("wrangler deploy --dry-run: {}", krate.upload()?));
        let state = work.join("state");
        let mut apply = self.shell.command("wrangler", &krate.path());
        apply.args(["d1", "migrations", "apply", "DB", "--local", "--persist-to"]).arg(&state);
        say(format!("applying with {}", Shell::name(&apply)));
        Shell::run(apply)?;
        let log = work.join("report.txt");
        let mut driver = self.shell.command(env!("CARGO"), self.shell.root());
        driver
            .args(["test", "-p", "uses-ink-cli", "--test", "hosted", "--", "--ignored", "--nocapture"])
            .env("HOSTED_APP", krate.path())
            .env("HOSTED_STATE", &state)
            .env("HOSTED_PORT", Case::App.port().to_string())
            .env("HOSTED_REPORT", &log);
        Shell::run(driver)?;
        let text = String::from_utf8_lossy(&Shell::read(&log)?).into_owned();
        for line in text.lines() {
            say(line.to_owned());
        }
        Ok(())
    }
}
