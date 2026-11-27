//! `farm workspace <name>`: a new jj workspace whose `target/` starts as a reflink copy of the
//! newest warm sibling's, so it builds in seconds, not the minutes of a cold compile. And
//! `--forget`, which removes one only when that cannot lose work.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use crate::name::Segment;
use crate::tree::Tree;
use crate::process::{Failure, Process};

/// How a new workspace got its `target/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Seeded {
    /// Reflinked from the workspace named: instant, and no space used.
    Reflink(PathBuf),
    /// Copied in full: the filesystem has no reflinks.
    Copy(PathBuf),
    /// No sibling has a `target/`: the first build is cold.
    Cold,
}

/// What `farm workspace` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Made {
    pub(crate) path: PathBuf,
    pub(crate) seeded: Seeded,
}

impl fmt::Display for Made {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "workspace {} ready; ", self.path.display())?;
        match &self.seeded {
            Seeded::Reflink(from) => write!(out, "target/ reflinked from {}", from.display()),
            Seeded::Copy(from) => write!(
                out,
                "no reflink here, so target/ was copied in full from {} (slow, and it uses the space)",
                from.display()
            ),
            Seeded::Cold => write!(out, "no sibling has a target/, so the first build is cold"),
        }
    }
}

/// The workspaces of the repo.
#[derive(Debug, Clone)]
pub(crate) struct Workspaces<'a> {
    tree: &'a Tree,
}

impl<'a> Workspaces<'a> {
    pub(crate) fn new(tree: &'a Tree) -> Self {
        Self { tree }
    }

    /// Adds the workspace `name` at `rev` (default `main`) and seeds its `target/`.
    pub(crate) fn make(&self, name: &Segment, rev: Option<&str>) -> Result<Made, Fault> {
        let container = self.tree.container();
        let path = container.join(name.as_str());
        if path.exists() {
            return Err(Fault::Exists { path });
        }
        std::fs::create_dir_all(&container).map_err(|source| Fault::Io { path: container.clone(), source })?;
        let mut jj = Command::new("jj");
        jj.arg("-R").arg(&self.tree.root).args(["workspace", "add", "--revision", rev.unwrap_or("main")]).arg(&path);
        Process::from(jj).text(&[]).map_err(|source| Fault::Jj { what: "workspace add", source })?;
        let seeded = match self.warmest(&path) {
            Some(from) => Self::seed(&from, &path)?,
            None => Seeded::Cold,
        };
        Ok(Made { path, seeded })
    }

    /// The sibling with the newest `target/`: another workspace, or the main checkout.
    fn warmest(&self, new: &Path) -> Option<PathBuf> {
        let container = self.tree.container();
        let mut homes: Vec<PathBuf> = std::fs::read_dir(&container)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .chain(container.parent().map(|parent| parent.join(self.tree.repo.as_str())))
            .chain([self.tree.root.clone()])
            .filter(|home| home != new)
            .collect();
        homes.sort();
        homes.dedup();
        homes
            .into_iter()
            .filter_map(|home| {
                let touched = home.join("target").metadata().and_then(|meta| meta.modified()).ok()?;
                Some((touched, home))
            })
            .max_by_key(|(touched, _): &(SystemTime, PathBuf)| *touched)
            .map(|(_, home)| home)
    }

    /// Copies `from/target` to `to/target`: a reflink, or in full when the filesystem has none.
    fn seed(from: &Path, to: &Path) -> Result<Seeded, Fault> {
        let copy = |mode: &str| {
            let mut cp = Command::new("cp");
            cp.args(["-a", mode]).arg(from.join("target")).arg(to.join("target"));
            Process::from(cp).text(&[])
        };
        if copy("--reflink=always").is_ok() {
            return Ok(Seeded::Reflink(from.to_path_buf()));
        }
        // a partial copy from the failed try would make the second one nest
        std::fs::remove_dir_all(to.join("target")).ok();
        copy("--reflink=auto").map_err(|source| Fault::Seed { from: from.to_path_buf(), source })?;
        Ok(Seeded::Copy(from.to_path_buf()))
    }

    /// Forgets the workspace `name` and removes its directory, if it is a workspace of this repo
    /// and holds nothing that is not in a commit.
    pub(crate) fn forget(&self, name: &Segment) -> Result<PathBuf, Fault> {
        let container = self.tree.container();
        let path = container.join(name.as_str());
        let missing = |_| Fault::Missing { path: path.clone() };
        let real = path.canonicalize().map_err(missing)?;
        let home = container.canonicalize().map_err(|source| Fault::Io { path: container.clone(), source })?;
        if real.parent() != Some(home.as_path()) || !real.is_dir() {
            return Err(Fault::Outside { path: real, container: home });
        }
        if real == self.tree.root {
            return Err(Fault::Current { name: name.clone() });
        }
        if Store::of(&real)? != Store::of(&self.tree.root)? {
            return Err(Fault::Foreign { path: real });
        }
        let mut diff = Command::new("jj");
        diff.arg("-R").arg(&real).args(["diff", "--summary"]);
        let changes = Process::from(diff).text(&[]).map_err(|source| Fault::Jj { what: "diff", source })?;
        if !changes.trim().is_empty() {
            return Err(Fault::Dirty { name: name.clone() });
        }
        let mut forget = Command::new("jj");
        forget.arg("-R").arg(&self.tree.root).args(["workspace", "forget"]).arg(name.as_str());
        Process::from(forget).text(&[]).map_err(|source| Fault::Jj { what: "workspace forget", source })?;
        std::fs::remove_dir_all(&real).map_err(|source| Fault::Io { path: real.clone(), source })?;
        Ok(real)
    }
}

/// The repo store a workspace belongs to: its `.jj/repo`, a directory in the main checkout and a
/// file pointing at it in every other workspace.
#[derive(Debug, PartialEq, Eq)]
struct Store(PathBuf);

impl Store {
    fn of(workspace: &Path) -> Result<Self, Fault> {
        let marker = workspace.join(".jj").join("repo");
        let io = |source| Fault::Io { path: marker.clone(), source };
        if marker.is_dir() {
            return marker.canonicalize().map(Self).map_err(io);
        }
        let pointer = std::fs::read_to_string(&marker).map_err(|_| Fault::Foreign { path: workspace.to_path_buf() })?;
        workspace.join(".jj").join(pointer.trim()).canonicalize().map(Self).map_err(io)
    }
}

/// Why a workspace could not be made or forgotten.
#[derive(Debug)]
pub(crate) enum Fault {
    Exists { path: PathBuf },
    Missing { path: PathBuf },
    /// The path is not a direct child of the repo's workspace directory.
    Outside { path: PathBuf, container: PathBuf },
    /// The workspace is the one the command runs in.
    Current { name: Segment },
    /// Not a jj workspace of this repo.
    Foreign { path: PathBuf },
    /// It holds changes that are not in a commit.
    Dirty { name: Segment },
    Jj { what: &'static str, source: Failure },
    Seed { from: PathBuf, source: Failure },
    Io { path: PathBuf, source: std::io::Error },
}

impl Fault {
    /// Whether the farm refused, rather than failed.
    pub(crate) const fn is_refusal(&self) -> bool {
        matches!(
            self,
            Self::Exists { .. } | Self::Outside { .. } | Self::Current { .. } | Self::Foreign { .. } | Self::Dirty { .. }
        )
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exists { path } => write!(out, "{} exists already", path.display()),
            Self::Missing { path } => write!(out, "no workspace at {}", path.display()),
            Self::Outside { path, container } => {
                write!(out, "{} is not directly inside {}; nothing was removed", path.display(), container.display())
            }
            Self::Current { name } => write!(out, "`{name}` is the workspace this command runs in; run it from another"),
            Self::Foreign { path } => write!(out, "{} is not a jj workspace of this repo; nothing was removed", path.display()),
            Self::Dirty { name } => write!(out, "workspace `{name}` has changes that are not committed; commit or abandon them first"),
            Self::Jj { what, .. } => write!(out, "`jj {what}` failed"),
            Self::Seed { from, .. } => write!(out, "copying target/ from {} failed", from.display()),
            Self::Io { path, .. } => write!(out, "cannot use {}", path.display()),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Jj { source, .. } | Self::Seed { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(base: &Path) -> Tree {
        let root = base.join("r-ws").join("here");
        std::fs::create_dir_all(&root).unwrap();
        Tree::locate(&root).unwrap()
    }

    fn base(name: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!("farm-ws-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        std::fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn the_warmest_sibling_is_the_one_with_the_newest_target() {
        let base = base("warm");
        let tree = tree(&base);
        let old = base.join("r-ws").join("old");
        let new = base.join("r-ws").join("new");
        for dir in [&old, &new] {
            std::fs::create_dir_all(dir.join("target")).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let made = base.join("r-ws").join("made");
        std::fs::create_dir_all(&made).unwrap();
        assert_eq!(Workspaces::new(&tree).warmest(&made).unwrap().file_name().unwrap(), "new");
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn seeding_copies_the_target_and_says_how() {
        let base = base("seed");
        let from = base.join("from");
        let to = base.join("to");
        std::fs::create_dir_all(from.join("target").join("debug")).unwrap();
        std::fs::write(from.join("target").join("debug").join("x"), "x").unwrap();
        std::fs::create_dir_all(&to).unwrap();
        let seeded = Workspaces::seed(&from, &to).unwrap();
        assert!(matches!(seeded, Seeded::Reflink(_) | Seeded::Copy(_)));
        assert_eq!(std::fs::read_to_string(to.join("target").join("debug").join("x")).unwrap(), "x");
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn forgetting_refuses_what_is_not_a_workspace_of_this_repo() {
        let base = base("forget");
        let tree = tree(&base);
        let plain = base.join("r-ws").join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        let spaces = Workspaces::new(&tree);
        assert!(matches!(spaces.forget(&"plain".parse().unwrap()), Err(Fault::Foreign { .. })));
        assert!(matches!(spaces.forget(&"nothing".parse().unwrap()), Err(Fault::Missing { .. })));
        assert!(matches!(spaces.forget(&"here".parse().unwrap()), Err(Fault::Current { .. })));
        assert!(plain.exists(), "a refusal removes nothing");
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn a_symlink_out_of_the_container_is_not_removed() {
        let base = base("link");
        let tree = tree(&base);
        let elsewhere = base.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, base.join("r-ws").join("sneaky")).unwrap();
        let err = Workspaces::new(&tree).forget(&"sneaky".parse().unwrap()).unwrap_err();
        assert!(matches!(err, Fault::Outside { .. }), "{err}");
        assert!(elsewhere.exists());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
