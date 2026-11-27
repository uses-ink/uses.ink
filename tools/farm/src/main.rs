//! `farm <verb>`: the build farm client. A repo's jobs and any cargo command run on the hosts of
//! the user's config, each in a slot of its own, so six agents do not make one machine thrash and
//! a test budget is measured on a quiet core. It knows no project: the jobs are the repo's own
//! `.config/farm.toml`. See docs/workflow.md for the verbs and docs/decisions.md (0047) for why
//! it is built this way.
//!
//! - [`cache`]: what the farm remembers on this machine: passes, running jobs, the journal, the
//!   result key and the digest of a job's inputs.
//! - [`cli`]: the command line, the exit codes, the console, errors and names, and the version.
//! - [`config`]: the hosts file, the repo's job file, the tree it sits in and its workspaces.
//! - [`host`]: [`host::Host`] and what a probe finds of it; [`host::Member`] and [`host::Fleet`],
//!   generic over the [`host::transport::Transport`] (local, ssh and the shell programs).
//! - [`job`]: a job, its events, its result and its log summary.
//! - [`place`]: [`place::Placement`], which host gets a job; how long jobs took there; waking hosts.
//! - [`run`]: [`run::Farm`], the verbs, one file each.

mod cache;
mod cli;
mod config;
mod host;
mod job;
mod place;
mod run;

use std::process::ExitCode;
use std::sync::Arc;

use cache::Cache;
use cache::inputs::{Inputs, Jj};
use cli::console::Console;
use cli::error::{Chain, Error};
use cli::exit::Exit;
use cli::{Command, Verb};
use config::hosts::{Config, Environment};
use config::jobs::Jobs;
use config::tree::Tree;
use config::workspace::Workspaces;
use host::transport::{Exec, Link, Local, Ssh};
use host::{Fleet, Member};
use host::{Host, Via};
use place::Fastest;
use run::{Farm, Setup};

/// The entry point.
#[derive(Debug, Clone, Copy)]
struct Entry;

impl Entry {
    /// Runs the farm command `args`, from the workspace the current directory is in; the exit
    /// code is the farm's own (see [`Exit`]).
    fn main(args: Vec<String>) -> ExitCode {
        let console = Arc::new(Console::default());
        let exit = match Self::exec(args) {
            Ok(exit) => exit,
            Err(error) => {
                console.fail(Chain(&error));
                error.exit()
            }
        };
        exit.into()
    }

    fn exec(args: Vec<String>) -> Result<Exit, Error> {
        let Command { flags, verb } = Command::parse(args)?;
        let console = Arc::new(Console::new(flags.verbose));
        if verb == Verb::Version {
            console.out(cli::version::Version::current());
            return Ok(Exit::Passed);
        }
        let here = std::env::current_dir().map_err(|source| Error::Io { path: ".".into(), source })?;
        let tree = Tree::find(&here)?;
        match &verb {
            Verb::Make(name) => {
                let made = Workspaces::new(&tree).make(name, flags.at.as_deref())?;
                console.out(made);
                return Ok(Exit::Passed);
            }
            Verb::Forget(name) => {
                let jobs = Jobs::load(&tree.root)?;
                let path = Workspaces::new(&tree).forget(name, jobs.disposable())?;
                console.out(format!("workspace `{name}` forgotten and {} removed", path.display()));
                return Ok(Exit::Passed);
            }
            _ => {}
        }
        let env = Environment::current();
        let config = Config::load(&env)?;
        let settings = Jobs::load_optional(&tree.root)?.map_or_default(|jobs| jobs.settings().clone());
        let cache = Self::cache(&env, &tree, &settings)?;
        let placement = Fastest::new(Self::table(cache.as_ref(), &console));
        let exec = Exec::new(Arc::clone(&console));
        let members = config.hosts.iter().map(|host| Self::member(host, &exec)).collect();
        let farm = Farm::new(Setup { fleet: Fleet::new(members), placement, console, flags, tree, settings, cache });
        farm.go(&verb)
    }

    /// The timings placement uses; a history that cannot be read is said and placement goes on
    /// without it.
    fn table(cache: Option<&Cache>, console: &Console) -> place::history::Table {
        let Some(cache) = cache else {
            return place::history::Table::default();
        };
        cache.history.load(&place::history::Wall).unwrap_or_else(|fault| {
            console.note(format!("timing history not used: {}", cache::Causes(&cache::Trouble::Store(fault))));
            place::history::Table::default()
        })
    }

    /// What the farm remembers on this machine, unless there is nowhere to keep it.
    fn cache(env: &Environment, tree: &Tree, settings: &config::jobs::Settings) -> Result<Option<Cache>, Error> {
        let Some(dir) = env.cache_dir(tree.repo.as_str()) else {
            return Ok(None);
        };
        let inputs =
            Inputs::new(Arc::new(Jj), &settings.exclude).map_err(|fault| Error::Cache(Box::new(fault.into())))?;
        Ok(Some(Cache::at(&dir, settings.limits, settings.memory, inputs)))
    }

    fn member(host: &Host, exec: &Exec) -> Member<Link> {
        let link = match &host.via {
            Via::Local => Link::Local(Local::new(exec.clone())),
            Via::Ssh(destination) => Link::Ssh(Ssh::new(exec.clone(), destination.clone())),
        };
        Member::new(host.clone(), link)
    }
}

fn main() -> ExitCode {
    Entry::main(std::env::args().skip(1).collect())
}
