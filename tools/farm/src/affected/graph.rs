//! The workspace's crates and who depends on whom, read from `cargo metadata --no-deps`.

use std::collections::BTreeSet;
use std::fmt;

use serde::Deserialize;

use crate::affected::Fault;

/// A package name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Package(String);

impl Package {
    /// A package named so.
    pub(crate) fn named(name: &str) -> Self {
        Self(name.to_owned())
    }
}

impl fmt::Display for Package {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// The top of `cargo metadata --format-version 1`: only what the graph reads.
#[derive(Debug, Deserialize)]
struct Metadata {
    workspace_root: String,
    packages: Vec<Entry>,
}

/// One workspace member.
#[derive(Debug, Deserialize)]
struct Entry {
    name: String,
    manifest_path: String,
    #[serde(default)]
    dependencies: Vec<Edge>,
}

/// One dependency of a member, of any kind: normal, dev or build.
#[derive(Debug, Deserialize)]
struct Edge {
    name: String,
}

/// A workspace member and where it lives.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Member {
    name: Package,
    /// Its directory, relative to the workspace root; empty for the root package.
    dir: String,
}

/// The crates of the workspace and the edges between them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Graph {
    members: Vec<Member>,
    /// For each member, the members that depend on it, by normal, dev or build edge.
    users: Vec<Vec<usize>>,
}

impl Graph {
    /// Reads the JSON of `cargo metadata --no-deps --format-version 1` for a workspace: its
    /// packages are the members, and a dependency named like a member is an edge to it.
    pub(crate) fn read(json: &str) -> Result<Self, Fault> {
        let Metadata { workspace_root, packages } = serde_json::from_str(json).map_err(Fault::Json)?;
        let root = workspace_root.trim_end_matches('/');
        let mut members = Vec::new();
        for entry in &packages {
            let dir = entry
                .manifest_path
                .strip_suffix("/Cargo.toml")
                .and_then(|dir| dir.strip_prefix(root))
                .map(|dir| dir.trim_start_matches('/').to_owned())
                .ok_or_else(|| Fault::Manifest {
                    package: Package::named(&entry.name),
                    path: entry.manifest_path.clone(),
                })?;
            members.push(Member { name: Package::named(&entry.name), dir });
        }
        let mut users = vec![Vec::new(); members.len()];
        for (user, entry) in packages.iter().enumerate() {
            for edge in &entry.dependencies {
                let used = members.iter().position(|member| member.name.0 == edge.name);
                if let Some(slot) = used.and_then(|used| users.get_mut(used)).filter(|slot| !slot.contains(&user)) {
                    slot.push(user);
                }
            }
        }
        Ok(Self { members, users })
    }

    /// How many crates the workspace has.
    pub(crate) const fn len(&self) -> usize {
        self.members.len()
    }

    /// The crate whose directory is the longest prefix of `path`.
    pub(crate) fn owner(&self, path: &str) -> Option<usize> {
        let holds = |dir: &str| dir.is_empty() || path.strip_prefix(dir).is_some_and(|rest| rest.starts_with('/'));
        self.members
            .iter()
            .enumerate()
            .filter(|(_, member)| holds(&member.dir))
            .max_by_key(|(_, member)| member.dir.len())
            .map(|(at, _)| at)
    }

    /// The seeds and every crate that depends on one of them, directly or not.
    pub(crate) fn closure(&self, seeds: &BTreeSet<usize>) -> BTreeSet<usize> {
        let mut reached = seeds.clone();
        let mut open: Vec<usize> = seeds.iter().copied().collect();
        while let Some(at) = open.pop() {
            for &user in self.users.get(at).map_or(&[][..], Vec::as_slice) {
                if reached.insert(user) {
                    open.push(user);
                }
            }
        }
        reached
    }

    /// The names of the crates at `at`, sorted.
    pub(crate) fn names(&self, at: &BTreeSet<usize>) -> Vec<Package> {
        let mut names: Vec<Package> =
            at.iter().filter_map(|&at| self.members.get(at)).map(|member| member.name.clone()).collect();
        names.sort();
        names
    }
}
