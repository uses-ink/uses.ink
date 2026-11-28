//! `farm <verb>`: the build farm client. A repo's jobs and any cargo command run on the hosts of
//! the user's config, each in a slot of its own, so six agents do not make one machine thrash and
//! a test budget is measured on a quiet core. It knows no project: the jobs are the repo's own
//! `.config/farm.toml`. See docs/workflow.md for the verbs and docs/decisions.md (0047) for why
//! it is built this way.
//!
//! - [`affected`]: the crates a change affects, so `farm test` runs only those.
//! - [`cache`]: what the farm remembers on this machine: passes, running jobs, the journal, the
//!   result key and the digest of a job's inputs.
//! - [`cli`]: the command line, the exit codes, the console, errors and names, and the version.
//! - [`config`]: the hosts file, the repo's job file, the tree it sits in and its workspaces.
//! - [`host`]: [`host::Host`] and what a probe finds of it; [`host::Member`] and [`host::Fleet`],
//!   generic over the [`host::transport::Transport`] (local, ssh and the shell programs).
//! - [`job`]: a job, its events, its result and its log summary.
//! - [`place`]: [`place::Placement`], which host gets a job; how long jobs took there; waking hosts.
//! - [`run`]: [`run::Farm`], the verbs, one file each.

mod affected;
mod cache;
mod cli;
mod config;
mod copies;
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
use cli::name::Segment;
use cli::{Command, Verb};
use config::hosts::{Config, Environment};
use config::jobs::Jobs;
use config::tree::Tree;
use config::workspace::Workspaces;
use copies::{Copies, Owners};
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
            Verb::Forget(name) => return Self::forget(name, &tree, &console),
            Verb::Prune => return Self::prune(&tree, &console, flags.yes),
            _ => {}
        }
        let env = Environment::current();
        let config = Config::load(&env)?;
        let settings = Jobs::load_optional(&tree.root)?.map_or_default(|jobs| jobs.settings().clone());
        let cache = Self::cache(&env, &tree, &settings)?;
        let placement = Fastest::new(Self::table(cache.as_ref(), &console));
        let farm = Farm::new(Setup {
            fleet: Self::fleet(&config, &console),
            placement,
            console,
            flags,
            tree,
            settings,
            cache,
        });
        farm.go(&verb)
    }

    /// Removes the workspace here, then its copy on every host that answers; a host that does not
    /// is said, and `farm workspace --prune` removes what it holds later.
    fn forget(name: &Segment, tree: &Tree, console: &Arc<Console>) -> Result<Exit, Error> {
        let jobs = Jobs::load(&tree.root)?;
        let path = Workspaces::new(tree).forget(name, jobs.disposable())?;
        console.out(format!("workspace `{name}` forgotten and {} removed", path.display()));
        let config = Config::load(&Environment::current()).inspect_err(|_| {
            console.note("the workspace is gone here, but its copies on the hosts were not looked at; `farm workspace --prune` removes them");
        })?;
        let removal = Copies::new(&Self::fleet(&config, console), tree).forget(name);
        if !removal.is_empty() {
            console.out(&removal);
        }
        Ok(removal.exit())
    }

    /// Lists the copies on the hosts that no workspace here owns, and with `yes` removes them.
    fn prune(tree: &Tree, console: &Arc<Console>, yes: bool) -> Result<Exit, Error> {
        let config = Config::load(&Environment::current())?;
        let fleet = Self::fleet(&config, console);
        let copies = Copies::new(&fleet, tree);
        let survey = copies.survey(&Owners::of(tree)?);
        console.out(&survey);
        if survey.is_empty() {
            return Ok(Exit::Passed);
        }
        if !yes {
            console.out("nothing was removed; run `farm workspace --prune --yes` to remove them");
            return Ok(Exit::Passed);
        }
        let removal = copies.prune(&survey);
        console.out(&removal);
        Ok(removal.exit())
    }

    /// The hosts of the config, each with the way to reach it.
    fn fleet(config: &Config, console: &Arc<Console>) -> Fleet<Link> {
        let exec = Exec::new(Arc::clone(console));
        Fleet::new(config.hosts.iter().map(|host| Self::member(host, &exec)).collect())
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
