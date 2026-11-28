//! `cargo xtask smoke`: the self-host guide (`docs/selfhost.md`) run as a stranger would run it,
//! on one machine, with the real `ink` binary: build a folder, serve it, serve it with a data
//! folder, sign in with the first-run link, enrol the command line, publish, make an
//! organization, ask for a domain, list previews and reports, take a backup and start from it,
//! and, when an older `ink` is named as the argument, start that one's data folder with this
//! one. The container is `release image`'s. Nothing leaves the machine: the names are
//! `*.localhost` ones, which `curl` is pointed at the loopback address.

mod cli;
mod fault;
mod server;
mod web;

use std::fmt;
use std::path::{Path, PathBuf};

use cli::Cli;
use server::Server;
use web::Call;

use crate::build::component::Components;
use crate::check::{Cargo, Command, Step, Target};
use crate::error::Error;
use crate::process::Process;
use crate::report::{Line, Report};

pub(crate) use fault::Fault;

/// The smoke run, and the older `ink` whose data folder it starts, when one is named.
#[derive(Debug, Clone)]
pub(crate) struct Smoke(pub(crate) Option<PathBuf>);

/// One stage of it, each a part of the guide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    /// `ink build` and `ink serve` of one folder.
    Folder,
    /// `ink serve --data` and the first-run link.
    Admin,
    /// `ink login --data`.
    Login,
    /// `ink init` and `ink push`, and the site served by its host name.
    Push,
    /// `ink org create`.
    Org,
    /// `ink domain add`.
    Domain,
    /// `ink preview list`.
    Preview,
    /// `ink abuse list`, with the operator's key.
    Abuse,
    /// A copy of the data folder, started again.
    Backup,
    /// An older data folder, started with this `ink`.
    Upgrade,
}

impl Stage {
    /// Every stage, in the order the guide has them.
    pub(crate) const ALL: [Self; 10] = [
        Self::Folder,
        Self::Admin,
        Self::Login,
        Self::Push,
        Self::Org,
        Self::Domain,
        Self::Preview,
        Self::Abuse,
        Self::Backup,
        Self::Upgrade,
    ];

    /// Its name in the report.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Admin => "admin",
            Self::Login => "login",
            Self::Push => "push",
            Self::Org => "org",
            Self::Domain => "domain",
            Self::Preview => "preview",
            Self::Abuse => "abuse",
            Self::Backup => "backup",
            Self::Upgrade => "upgrade",
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// The run: where it works, the `ink` it runs and the server it keeps.
#[derive(Debug)]
struct Scene {
    work: PathBuf,
    port: u16,
    cli: Cli,
    older: Option<PathBuf>,
    server: Option<Server>,
}

impl Smoke {
    const BUILD: Cargo<[&'static str; 1]> =
        Cargo { command: Command::Build, scope: ["uses-ink-cli"], flags: &[], target: Target::Host };
}

impl Step for Smoke {
    fn run(&self, root: &Path, report: &mut Report) {
        let before = report.failures().len();
        Components.run(root, report);
        if report.failures().len() > before {
            return;
        }
        Self::BUILD.run(root, report);
        if report.failures().len() > before {
            return;
        }
        let mut scene = match Scene::new(root, self.0.clone()) {
            Ok(scene) => scene,
            Err(fault) => return report.fail(Error::Smoked { stage: Stage::Folder, fault }),
        };
        for stage in Stage::ALL {
            match scene.play(stage) {
                Ok(text) => report.line(Line::Smoked { stage, text }),
                Err(fault) => return report.fail(Error::Smoked { stage, fault }),
            }
        }
    }
}

impl fmt::Display for Smoke {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "the self-host guide run end to end")
    }
}

impl Scene {
    /// The name the first-run link and the dashboard are on.
    const API: &'static str = "api.localhost";

    /// The handle the guide publishes under.
    const HANDLE: &'static str = "garden";

    /// An empty working folder under `target/smoke`, and a free port.
    fn new(root: &Path, older: Option<PathBuf>) -> Result<Self, Fault> {
        let work = root.join("target/smoke");
        let disk = |source| Fault::Disk { path: work.clone(), source };
        match std::fs::remove_dir_all(&work) {
            Err(source) if source.kind() != std::io::ErrorKind::NotFound => return Err(disk(source)),
            Ok(()) | Err(_) => {}
        }
        std::fs::create_dir_all(&work).map_err(disk)?;
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|socket| socket.local_addr())
            .map_err(Fault::Port)?
            .port();
        let cli = Cli::new(&root.join("target/debug/ink"), &work.join("home"));
        Ok(Self { work, port, cli, older, server: None })
    }

    fn api(&self) -> String {
        format!("http://{}:{}", Self::API, self.port)
    }

    fn site(&self) -> PathBuf {
        self.work.join("site")
    }

    fn address(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }

    fn guard(what: &'static str, wanted: &str, found: &str) -> Result<(), Fault> {
        if found.contains(wanted) { Ok(()) } else { Err(Fault::expected(what, format!("`{wanted}`"), found)) }
    }

    /// Writes `text` to `path`, below the working folder.
    fn write(path: &Path, text: &str) -> Result<(), Fault> {
        std::fs::write(path, text).map_err(|source| Fault::Disk { path: path.to_path_buf(), source })
    }

    fn make(path: &Path) -> Result<(), Fault> {
        std::fs::create_dir_all(path).map_err(|source| Fault::Disk { path: path.to_path_buf(), source })
    }

    /// An answer with the status wanted and a word in its body.
    fn answer(reply: &web::Reply, status: u16, word: &str) -> Result<(), Fault> {
        if reply.status != status {
            return Err(Fault::expected(
                "the status",
                status.to_string(),
                &format!("{}: {}", reply.status, reply.body),
            ));
        }
        Self::guard("the body", word, &reply.body)
    }

    /// Starts `ink serve` the way the guide does, in the site folder, with `more` after the address.
    fn start(&mut self, log: &str, more: &[&str]) -> Result<String, Fault> {
        let address = self.address();
        let mut args = vec!["serve", "--addr", &address];
        args.extend(more);
        let server = Server::start(self.cli.bin(), (&self.site(), &self.work.join(log)), &args)?;
        let said = server.said()?;
        self.server = Some(server);
        Ok(said)
    }

    fn stop(&mut self) -> Result<(), Fault> {
        self.server.take().map_or(Ok(()), Server::stop)
    }

    /// Runs one stage; what it checked, as a line.
    fn play(&mut self, stage: Stage) -> Result<String, Fault> {
        match stage {
            Stage::Folder => self.folder(),
            Stage::Admin => self.admin(),
            Stage::Login => self.login(),
            Stage::Push => self.push(),
            Stage::Org => self.org(),
            Stage::Domain => self.domain(),
            Stage::Preview => self.preview(),
            Stage::Abuse => self.abuse(),
            Stage::Backup => self.backup(),
            Stage::Upgrade => self.upgrade(),
        }
    }

    fn folder(&mut self) -> Result<String, Fault> {
        let site = self.site();
        Self::make(&site)?;
        Self::write(&site.join("index.md"), "# Hello\n")?;
        self.cli.run(&site, &["build"])?;
        let said = self.start("folder.log", &[])?;
        Self::guard("the start's report", "needs --data", &said)?;
        let page = Call::new("127.0.0.1", self.port, "/").get()?;
        Self::answer(&page, 200, "<title>Hello</title>")?;
        self.stop()?;
        Ok("`ink build` then `ink serve` answered / with the page, `api` off".to_owned())
    }

    /// The first-run link of a start's report: its path, since the host is the API's. The report
    /// writes it alone on a line of the `admin link` block.
    fn link(said: &str) -> Result<String, Fault> {
        said.split_whitespace()
            .find(|word| word.starts_with("http") && word.contains("/login/"))
            .and_then(|url| url.find("/login/").and_then(|at| url.get(at..)))
            .map(str::to_owned)
            .ok_or_else(|| Fault::expected("the start's report", "an `admin link` block with a /login/ link", said))
    }

    fn admin(&mut self) -> Result<String, Fault> {
        let said = self.start("state.log", &["--data", "../state"])?;
        let path = Self::link(&said)?;
        let jar = self.work.join("jar");
        let link = Call::new(Self::API, self.port, &path);
        Self::answer(&link.get()?, 200, "Sign in")?;
        // a form with the address, from the dashboard's own origin: what the page's button posts
        let signed = link.with(&jar).post(&[("address", "admin@example.org")])?;
        if signed.status != 303 {
            return Err(Fault::expected(
                "signing in with the link",
                "303",
                &format!("{}: {}", signed.status, signed.body),
            ));
        }
        let account = Call::new(Self::API, self.port, "/account").with(&jar).get()?;
        Self::answer(&account, 200, "Your sites")?;
        Ok("the first-run link shows a sign-in form, and posting an address to it signs in the owner".to_owned())
    }

    fn login(&mut self) -> Result<String, Fault> {
        let api = self.api();
        let said = self.cli.run(&self.work, &["login", "--data", "./state", "--api", &api])?;
        Self::guard("`ink login`", "enrolled", &said)?;
        self.cli = self.cli.with_api(&api);
        Ok("`ink login --data ./state --api` enrolled the operator key".to_owned())
    }

    fn push(&mut self) -> Result<String, Fault> {
        let made = self.cli.run(&self.work, &["-C", "site", "init", "--handle", Self::HANDLE])?;
        Self::guard("`ink init`", "made, and linked", &made)?;
        let deploy = self.cli.run(&self.work, &["-C", "site", "push"])?;
        if deploy.trim().len() != 64 {
            return Err(Fault::expected("`ink push`", "the deploy id on stdout", &deploy));
        }
        let host = format!("{}.localhost", Self::HANDLE);
        let page = Call::new(&host, self.port, "/").get()?;
        Self::answer(&page, 200, &format!("href=\"http://{host}:{}/\"", self.port))?;
        Self::answer(&page, 200, "<title>Hello</title>")?;
        Ok(format!("`ink init`, `ink push`, and {host} serves the deploy"))
    }

    fn org(&mut self) -> Result<String, Fault> {
        let made = self.cli.run(&self.work, &["org", "create", "Garden"])?;
        Self::guard("`ink org create`", "role     owner", &made)?;
        let listed = self.cli.run(&self.work, &["org", "list"])?;
        Self::guard("`ink org list`", "Garden", &listed)?;
        Ok("`ink org create Garden` made it and `ink org list` shows it".to_owned())
    }

    fn domain(&mut self) -> Result<String, Fault> {
        let added = self.cli.run(&self.work, &["-C", "site", "domain", "add", "www.example.org"])?;
        Self::guard("`ink domain add`", "_uses-ink.www.example.org TXT \"uses-ink-domain=", &added)?;
        Ok("`ink domain add www.example.org` says which TXT record to publish".to_owned())
    }

    fn preview(&mut self) -> Result<String, Fault> {
        let listed = self.cli.run(&self.work, &["-C", "site", "preview", "list"])?;
        Self::guard("`ink preview list`", "no previews", &listed)?;
        Ok("`ink preview list` says there are no previews".to_owned())
    }

    fn abuse(&mut self) -> Result<String, Fault> {
        let listed = self.cli.run(&self.work, &["abuse", "list"])?;
        Self::guard("`ink abuse list`", "no reports", &listed)?;
        Ok("`ink abuse list` with the operator key says there are no reports".to_owned())
    }

    fn backup(&mut self) -> Result<String, Fault> {
        self.stop()?;
        let (state, copy) = (self.work.join("state"), self.work.join("backup"));
        let mut command = std::process::Command::new("cp");
        command.arg("-a").arg(&state).arg(&copy);
        let line = format!("cp -a {} {}", state.display(), copy.display());
        Process::from(command).stdout(&[]).map_err(|source| Fault::Command { line, source })?;
        let said = self.start("backup.log", &["--data", "../backup"])?;
        Self::guard("the start's report", "not made", &said)?;
        Self::guard("the start's report", "none was due", &said)?;
        let host = format!("{}.localhost", Self::HANDLE);
        Self::answer(&Call::new(&host, self.port, "/").get()?, 200, "<title>Hello</title>")?;
        // the session of the first-run link is in the copy, and so are the account's site and organization
        let jar = self.work.join("jar");
        let account = Call::new(Self::API, self.port, "/account").with(&jar).get()?;
        Self::answer(&account, 200, Self::HANDLE)?;
        Self::guard("the account page", "Garden", &account.body)?;
        self.stop()?;
        Ok("a copy of the data folder started: the site, the account and its session survive, no second admin link"
            .to_owned())
    }

    fn upgrade(&mut self) -> Result<String, Fault> {
        let Some(older) = self.older.clone() else {
            return Ok(
                "not run: give the path of an older `ink` as the argument to start its data folder with this one"
                    .to_owned(),
            );
        };
        let (dir, api) = (self.work.join("older"), self.api());
        let (site, old) = (dir.join("site"), Cli::new(&older, &dir.join("home")).with_api(&api));
        Self::make(&site)?;
        Self::write(&site.join("index.md"), "# Older\n")?;
        old.run(&site, &["build"])?;
        let address = self.address();
        let args = ["serve", "--addr", &address, "--data", "../state"];
        let server = Server::start(old.bin(), (&site, &dir.join("older.log")), &args)?;
        old.run(&dir, &["login", "--data", "./state", "--api", &api])?;
        old.run(&dir, &["-C", "site", "init", "--handle", "older"])?;
        old.run(&dir, &["-C", "site", "push"])?;
        server.stop()?;
        let (site_folder, current) = (self.site(), dir.join("current.log"));
        let args = ["serve", "--addr", &address, "--data", "../older/state"];
        let server = Server::start(self.cli.bin(), (&site_folder, &current), &args)?;
        let said = server.said()?;
        Self::guard("the start's report", ".sql", &said)?;
        let page = Call::new("older.localhost", self.port, "/").get()?;
        Self::answer(&page, 200, "<title>Older</title>")?;
        server.stop()?;
        Ok("an older data folder started with this `ink`: migrations applied and its site served".to_owned())
    }
}
