//! Where a parsed command line becomes a command that runs: the site's folder from `-C` is
//! handed to each command that reads one, and the verbs that are not folders (help, completions,
//! the manual, the verbs not built yet) are run here.

use std::path::PathBuf;

use serde::Serialize;

use crate::args::{Command, Completions, Deployment, Explain, Serve, Topic};
use crate::error::Error;
use crate::exit::Exit;
use crate::help::Help;
use crate::man::Manual;
use crate::output::Io;
use crate::settings::{Process, Settings};
use crate::site::DEFAULT_OUT;
use crate::tree::Tree;

/// What `ink help --json` prints.
#[derive(Debug, Serialize)]
struct Said<'a> {
    schema: &'static str,
    text: &'a str,
}

/// What `ink completions --json` and `ink man --json` print: the script or the page.
#[derive(Debug, Serialize)]
struct Script<'a> {
    schema: &'static str,
    name: &'a str,
    #[serde(rename = "script")]
    content: &'a str,
}

impl Explain {
    /// The same command with its address folded: `ink explain [PATH] --why CODE` is the words
    /// `[PATH, CODE]` the rest of the command reads, and plain `explain` keeps its path.
    fn addressed(mut self, dir: PathBuf) -> Self {
        self.dir = dir;
        if let Some(code) = self.why.take() {
            self.why = Some(self.path.take().into_iter().chain(code).collect());
        }
        self
    }
}

impl Serve {
    /// The same command with its folder defaulted to the site's `.ink/out`.
    fn sited(mut self, dir: &std::path::Path) -> Self {
        if self.folder.as_os_str().is_empty() {
            self.folder = dir.join(DEFAULT_OUT);
        }
        self
    }
}

impl Completions {
    /// Prints the script for the shell.
    fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let script = self.shell.script(&Tree::ink().command());
        io.line(script.trim_end())?;
        io.json(&Script { schema: "uses.script/v1", name: &self.shell.name(), content: &script })?;
        Ok(Exit::Done)
    }
}

impl Topic {
    /// Prints the help of a topic.
    fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let help = Help::new(Tree::ink(), io.prose(), crate::help::Columns::of(&Process));
        let text = help.topic(&self.words).map_err(|source| Error::Help { source })?;
        io.painted(text.trim_end())?;
        io.json(&Said { schema: "uses.help/v1", text: &text })?;
        Ok(Exit::Done)
    }
}

impl Command {
    /// Runs the command in the site's folder `site`.
    #[expect(clippy::too_many_lines, reason = "one arm for each command")]
    pub(crate) fn run(self, site: PathBuf, io: &mut Io<'_>) -> Result<Exit, Error> {
        match self {
            Self::Check(mut check) => {
                check.dir = site;
                check.run(io)
            }
            Self::Build(mut build) => {
                build.dir = site;
                build.run(io)
            }
            Self::Preview(mut preview) => {
                preview.dir = site;
                preview.run(io)
            }
            Self::Serve(serve) => serve.sited(&site).run(io),
            Self::Explain(explain) => explain.addressed(site).run(io),
            Self::Pin(mut pin) => {
                pin.dir = site;
                pin.run(io)
            }
            Self::Select(mut select) => {
                select.dir = site;
                select.run(io)
            }
            Self::Deploy(deploy) => match deploy.command {
                Deployment::List(list) => list.run(io),
                Deployment::Show(show) => show.run(io),
                Deployment::Rollback(rollback) => rollback.run(io),
                Deployment::Resume(remote) => remote.resume(io),
            },
            Self::Deploys(list) => list.run(io),
            Self::Completions(completions) => completions.run(io),
            Self::Man => {
                let page = Manual::of(Tree::ink()).render().map_err(|source| Error::Manual { source })?;
                io.line(page.trim_end())?;
                io.json(&Script { schema: "uses.script/v1", name: "man", content: &page })?;
                Ok(Exit::Done)
            }
            Self::Help(topic) => topic.run(io),
            Self::Init => Err(Error::Pending { verb: "init" }),
            Self::Login(login) => login.run(io),
            Self::Logout(server) => server.logout(io),
            Self::Whoami(server) => server.whoami(io),
            Self::Key(key) => key.command.run(io),
            Self::Push => Err(Error::Pending { verb: "push" }),
            Self::Status => Err(Error::Pending { verb: "status" }),
            Self::Watch => Err(Error::Pending { verb: "watch" }),
            Self::Export => Err(Error::Pending { verb: "export" }),
        }
    }
}

impl crate::args::Cli {
    /// The terminal settings of this run: the flags, the environment, the user's config file,
    /// the defaults, in that order. The terminal layer reads these.
    pub fn settings(&self) -> Result<Settings, Error> {
        let preferences =
            crate::settings::Preferences::load(&Process).map_err(|source| Error::Preferences { source })?;
        Ok(Settings::of(self, &preferences))
    }
}
