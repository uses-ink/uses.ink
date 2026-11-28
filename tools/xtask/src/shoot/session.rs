//! One run of `cargo xtask shoot`: the workspace, the host and the servers it started.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::fault::Fault;
use super::flags::Flags;
use super::host::Host;
use super::plan::{Plan, Shot, Static};
use super::serve::{Served, Servers};
use super::target::{Folder, Target};
use crate::report::{Line, Report};

/// What a run holds while it takes pictures.
#[derive(Debug)]
pub(super) struct Session {
    root: PathBuf,
    host: Host,
    flags: Flags,
    servers: Servers,
}

impl Session {
    /// The port the mock is served on, and the dashboard's.
    const MOCK: u16 = 8745;
    const DASH: u16 = 8744;

    /// A run in the workspace at `root`.
    pub(super) fn new(root: &Path, flags: &Flags) -> Self {
        Self { root: root.to_path_buf(), host: Host::new(root), flags: flags.clone(), servers: Servers::default() }
    }

    /// Says something in the report.
    fn say(report: &mut Report, text: String) {
        report.line(Line::Shoot { text });
    }

    /// The `ink` binary, built.
    fn ink(&self) -> Result<PathBuf, Fault> {
        let mut cargo = Command::new(env!("CARGO"));
        cargo.args(["build", "-p", "uses-ink-cli"]).current_dir(&self.root);
        Host::run(cargo)?;
        Ok(self.root.join("target/debug/ink"))
    }

    /// Puts the lockfile in the report between markers, as the farm's log is the way back.
    pub(super) fn lock(&self, report: &mut Report) -> Result<(), Fault> {
        let text = self.host.resolve()?;
        Self::say(
            report,
            format!(
                "=====BEGIN tools/shoot/package-lock.json=====\n{text}\n=====END tools/shoot/package-lock.json====="
            ),
        );
        Ok(())
    }

    /// Installs the packages and the browser. A failed download is said, not fatal: the driver
    /// tries the static build of `@sparticuz/chromium` next and fails with every cause if none starts.
    fn prepare(&self, report: &mut Report) -> Result<(), Fault> {
        self.host.install()?;
        if let Err(fault) = self.host.fetch() {
            Self::say(report, format!("browser download failed, trying the static build: {fault}"));
        }
        Ok(())
    }

    /// Whether a browser starts here.
    pub(super) fn probe(&mut self, report: &mut Report) -> Result<(), Fault> {
        self.prepare(report)?;
        self.host.drive(&Plan {
            probe: true,
            out: self.root.join(Target::OUT),
            widths: Vec::new(),
            shots: Vec::new(),
            serve: None,
        })
    }

    /// Takes the pictures of `shots`, and stops the servers whatever happened.
    fn finish(&mut self, report: &mut Report, shots: Vec<Shot>, serve: Option<Static>) -> Result<(), Fault> {
        let plan =
            Plan { probe: false, out: self.root.join(Target::OUT), widths: self.flags.widths.clone(), shots, serve };
        let driven = self.prepare(report).and_then(|()| self.host.drive(&plan));
        let stopped = std::mem::take(&mut self.servers).stop();
        driven?;
        stopped?;
        Self::say(report, format!("pictures are in {}", self.root.join(Target::OUT).display()));
        Ok(())
    }

    /// The landing page and the docs, built and served.
    pub(super) fn site(&mut self, report: &mut Report) -> Result<(), Fault> {
        self.ink()?;
        let mut shots = Vec::new();
        for folder in Folder::ALL {
            folder.build(&self.root)?;
            self.servers.add(folder.serve(&self.root)?);
            shots.extend(folder.shots(&self.root)?);
            Self::say(report, format!("{} built and served on {}", folder.dir, folder.address()));
        }
        self.finish(report, shots, None)
    }

    /// The dashboard of `ink serve --data`, signed out.
    pub(super) fn dashboard(&mut self, report: &mut Report) -> Result<(), Fault> {
        let ink = self.ink()?;
        let home = self.root.join(Host::HOME);
        for dir in ["data", "sites"] {
            std::fs::create_dir_all(home.join(dir)).map_err(|source| Fault::File { path: home.join(dir), source })?;
        }
        let address = SocketAddr::from(([127, 0, 0, 1], Self::DASH));
        let mut command = Command::new(ink);
        command
            .arg("serve")
            .arg(home.join("sites"))
            .args(["--addr", &address.to_string(), "--api-host", "api.localhost", "--origin"])
            .arg(format!("http://api.localhost:{}", Self::DASH))
            .arg("--data")
            .arg(home.join("data"));
        self.servers.add(Served::start(command, "dashboard", address, &home.join("dashboard.log"))?);
        let url = format!("http://api.localhost:{}/", Self::DASH);
        self.finish(report, vec![Shot { name: "dashboard-home".to_owned(), url }], None)
    }

    /// The mock's pages, served by the driver from `--from`.
    pub(super) fn mock(&mut self, report: &mut Report) -> Result<(), Fault> {
        let from = self.root.join(self.flags.from.as_deref().unwrap_or_else(|| Path::new("tools/shoot/mock")));
        if !from.join("index.html").is_file() {
            return Err(Fault::Mock { path: from });
        }
        let port = Self::MOCK;
        let page = |name: &str, route: &str| Shot {
            name: format!("mock-{name}"),
            url: format!("http://127.0.0.1:{port}/{route}"),
        };
        let shots =
            vec![page("landing", "index.html"), page("reader", "reader/index.html"), page("post", "reader/post.html")];
        self.finish(report, shots, Some(Static { root: from, port }))
    }
}
