//! What `cargo xtask shoot` can be asked for.

use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use super::fault::Fault;
use super::flags::Flags;
use super::host::Host;
use super::plan::Shot;
use super::serve::Served;
use super::session::Session;
use crate::error::Error;
use crate::report::Report;

/// The pages of a site that is built and served, and the port it is served on.
#[derive(Debug, Clone, Copy)]
pub(super) struct Folder {
    /// The name of its pictures and of its build folder.
    pub(super) name: &'static str,
    /// The site's folder, relative to the root.
    pub(super) dir: &'static str,
    /// The loopback port `ink serve` listens on: one each, so two runs on one host do not collide.
    port: u16,
}

impl Folder {
    /// The sites the repository publishes (decisions 0120 and 0171).
    pub(super) const ALL: [Self; 2] =
        [Self { name: "landing", dir: "landing", port: 8741 }, Self { name: "docs", dir: "docs", port: 8742 }];

    /// The pages of the documentation site that are pictured: the home page, the longest page, a
    /// decision page, and the decisions index; each as its name and its route.
    const DOCS: [(&'static str, &'static str); 3] = [("home", "/"), ("cli", "/cli"), ("decisions", "/decisions")];

    /// The decision pictured, by its number.
    const DECISION: &'static str = "0180";

    /// Where its build is written, relative to the root.
    fn out(self, root: &Path) -> PathBuf {
        root.join(Host::HOME).join("out").join(self.name)
    }

    /// Where it is served.
    pub(super) fn address(self) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], self.port))
    }

    /// `ink build` of the site, the way the gate builds it.
    pub(super) fn build(self, root: &Path) -> Result<(), Fault> {
        let mut command = Command::new(root.join("target/debug/ink"));
        command
            .arg("-C")
            .arg(root.join(self.dir))
            .args(["build", "--source", "folder", "-o"])
            .arg(self.out(root))
            .arg("--cache")
            .arg(root.join(Host::HOME).join("cache").join(self.name));
        Host::run(command)
    }

    /// `ink serve` of its build.
    pub(super) fn serve(self, root: &Path) -> Result<Served, Fault> {
        let mut command = Command::new(root.join("target/debug/ink"));
        command
            .arg("serve")
            .arg(self.out(root))
            .args(["--host-mode", "single", "--addr"])
            .arg(self.address().to_string());
        Served::start(command, self.name, self.address(), &root.join(Host::HOME).join(format!("{}.log", self.name)))
    }

    /// The pages it has pictures of.
    pub(super) fn shots(self, root: &Path) -> Result<Vec<Shot>, Fault> {
        let url = |route: &str| format!("http://{}{route}", self.address());
        let shot = |name: String, route: &str| Shot { name, url: url(route) };
        if self.name == "landing" {
            return Ok(vec![shot("landing".to_owned(), "/")]);
        }
        let mut shots: Vec<Shot> = Self::DOCS.iter().map(|(name, route)| shot(format!("docs-{name}"), route)).collect();
        shots.push(shot(format!("docs-{}", Self::DECISION), &Self::decision(root)?));
        Ok(shots)
    }

    /// The route of the decision pictured: its file name without the extension.
    fn decision(root: &Path) -> Result<String, Fault> {
        let dir = root.join("docs/decisions");
        let file = |source| Fault::File { path: dir.clone(), source };
        for entry in std::fs::read_dir(&dir).map_err(file)? {
            let name = entry.map_err(file)?.file_name().to_string_lossy().into_owned();
            if let Some(stem) = name.strip_suffix(".md").filter(|stem| stem.starts_with(Self::DECISION)) {
                return Ok(format!("/decisions/{stem}"));
            }
        }
        Err(Fault::Decision { number: Self::DECISION })
    }
}

/// What `cargo xtask shoot` takes pictures of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// The landing page and the documentation site, built by `ink` and served by `ink serve`.
    Site,
    /// `ink serve` with a data folder: the dashboard's pages that need no sign-in.
    Dashboard,
    /// The design mock, served as it is, for a reference from the same browser (`--from <folder>`).
    Mock,
    /// Whether a browser starts on this host, and which.
    Probe,
    /// Resolves the npm packages and prints the lockfile, to be committed.
    Lock,
}

impl Target {
    /// Every target, in the order they are listed.
    pub(crate) const ALL: [Self; 5] = [Self::Site, Self::Dashboard, Self::Mock, Self::Probe, Self::Lock];

    /// Where the pictures are written, relative to the root.
    pub(crate) const OUT: &'static str = "target/shots";

    /// Its name on the command line.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Site => "site",
            Self::Dashboard => "dashboard",
            Self::Mock => "mock",
            Self::Probe => "probe",
            Self::Lock => "lock",
        }
    }

    /// Runs it, its lines into `report`.
    pub(super) fn run(self, root: &Path, flags: &Flags, report: &mut Report) -> Result<(), Fault> {
        let mut session = Session::new(root, flags);
        match self {
            Self::Lock => session.lock(report),
            Self::Probe => session.probe(report),
            Self::Site => session.site(report),
            Self::Dashboard => session.dashboard(report),
            Self::Mock => session.mock(report),
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

impl FromStr for Target {
    type Err = Error;

    fn from_str(given: &str) -> Result<Self, Error> {
        Self::ALL
            .into_iter()
            .find(|target| target.name() == given)
            .ok_or_else(|| Error::Target { given: given.to_owned() })
    }
}
