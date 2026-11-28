//! A workspace's copies on the hosts: `farm workspace --forget` removes them with the workspace,
//! and `--prune` removes the ones no workspace here owns. Both go through the host's own
//! `Remove` program, which refuses any path that resolves outside the farm's directory.

mod report;
#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::thread;

pub(crate) use report::{Removal, Survey};

use crate::cli::error::Error;
use crate::cli::name::{HostName, Segment};
use crate::config::tree::Tree;
use crate::host::transport::Transport;
use crate::host::{Fleet, Member};
use crate::job::event::Gone;

/// Where a copy is and what is known of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Stray {
    pub(crate) host: HostName,
    pub(crate) name: Segment,
    /// Its path on the host, as the person would type it.
    pub(crate) path: String,
    pub(crate) kib: u64,
}

/// What became of one copy on one host.
#[derive(Debug)]
pub(crate) enum Outcome {
    Gone(Gone),
    /// The host did not answer, so the copy may still be there.
    Unreached(Error),
}

/// A host's answer about one copy.
#[derive(Debug)]
pub(crate) struct Reached {
    pub(crate) host: HostName,
    pub(crate) path: String,
    pub(crate) outcome: Outcome,
}

/// The names of the workspaces this machine has: each directory beside the main checkout, the
/// main checkout, and the one the command runs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Owners(HashSet<String>);

impl Owners {
    /// Reads the workspace directory of `tree`.
    pub(crate) fn of(tree: &Tree) -> Result<Self, Error> {
        let mut names: HashSet<String> = [tree.workspace.to_string(), "main".to_owned()].into();
        let container = tree.container();
        match std::fs::read_dir(&container) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry.map_err(|source| Error::Io { path: container.clone(), source })?;
                    names.extend(entry.file_name().to_str().map(str::to_owned));
                }
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(Error::Io { path: container, source }),
        }
        Ok(Self(names))
    }

    fn owns(&self, name: &Segment) -> bool {
        self.0.contains(name.as_str())
    }
}

/// The copies of one repo on every host that keeps a copy of its own: this machine is not one.
#[derive(Debug)]
pub(crate) struct Copies<'a, T> {
    fleet: &'a Fleet<T>,
    tree: &'a Tree,
}

impl<'a, T: Transport + Sync> Copies<'a, T> {
    pub(crate) fn new(fleet: &'a Fleet<T>, tree: &'a Tree) -> Self {
        Self { fleet, tree }
    }

    /// The members that hold copies, each with a way to say where one lives.
    fn remote(&self) -> impl Iterator<Item = &'a Member<T>> {
        self.fleet.members().iter().filter(|member| !member.is_local())
    }

    fn path(&self, member: &Member<T>, name: &Segment) -> String {
        format!("{}/{}/{name}", member.host.dir, self.tree.repo)
    }

    /// Asks every host, at once, to remove the copy of workspace `name`.
    pub(crate) fn forget(&self, name: &Segment) -> Removal {
        let names = [name];
        Removal::new(self.remove_all(|_| names.to_vec()))
    }

    /// Runs the removals `wanted` names for each host, the hosts at once, a host's one by one.
    fn remove_all<'n>(&self, wanted: impl Fn(&Member<T>) -> Vec<&'n Segment> + Sync) -> Vec<Reached> {
        let wanted = &wanted;
        thread::scope(|scope| {
            let running: Vec<_> = self
                .remote()
                .map(|member| {
                    let names = wanted(member);
                    let handle = scope.spawn(move || {
                        names.iter().map(|name| self.remove_one(member, name)).collect::<Vec<Reached>>()
                    });
                    (member, wanted(member), handle)
                })
                .collect();
            let mut reached = Vec::new();
            for (member, names, handle) in running {
                match handle.join() {
                    Ok(done) => reached.extend(done),
                    Err(_) => reached.extend(names.iter().map(|name| self.crashed(member, name))),
                }
            }
            reached
        })
    }

    fn remove_one(&self, member: &Member<T>, name: &Segment) -> Reached {
        let outcome = match member.remove(&self.tree.repo, name) {
            Ok(gone) => Outcome::Gone(gone),
            Err(error) => Outcome::Unreached(error),
        };
        Reached { host: member.host.name.clone(), path: self.path(member, name), outcome }
    }

    /// A removal whose thread panicked: said, not lost.
    fn crashed(&self, member: &Member<T>, name: &Segment) -> Reached {
        let label = format!("removing {name} on `{}`", member.host.name);
        let outcome = Outcome::Unreached(Error::Crashed { label });
        Reached { host: member.host.name.clone(), path: self.path(member, name), outcome }
    }

    /// Lists, on every host, the copies that `owners` does not own.
    pub(crate) fn survey(&self, owners: &Owners) -> Survey {
        let listed: Vec<_> = thread::scope(|scope| {
            let running: Vec<_> =
                self.remote().map(|member| (member, scope.spawn(move || member.copies(&self.tree.repo)))).collect();
            running.into_iter().map(|(member, handle)| (member, handle.join())).collect()
        });
        let mut survey = Survey::default();
        for (member, joined) in listed {
            let host = member.host.name.clone();
            match joined {
                Ok(Ok(found)) => {
                    for copy in found.into_iter().filter(|copy| !owners.owns(&copy.name)) {
                        let path = self.path(member, &copy.name);
                        survey.strays.push(Stray { host: host.clone(), name: copy.name, path, kib: copy.kib });
                    }
                }
                Ok(Err(error)) => survey.missed.push((host, error)),
                Err(_) => survey.missed.push((host.clone(), Error::Crashed { label: format!("listing `{host}`") })),
            }
        }
        survey
    }

    /// Removes every copy the survey found.
    pub(crate) fn prune(&self, survey: &Survey) -> Removal {
        let wanted = |member: &Member<T>| {
            survey.strays.iter().filter(|stray| stray.host == member.host.name).map(|stray| &stray.name).collect()
        };
        Removal::new(self.remove_all(wanted))
    }
}
