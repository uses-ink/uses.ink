//! Which workspace this is: the repo's name and the workspace's, which name its directory on
//! every host.

use std::path::{Path, PathBuf};

use crate::cli::error::Error;
use crate::cli::name::Segment;
use crate::host::{Host, Via};

/// A jj workspace of the repo: `<repo>-ws/<name>` beside the main checkout `<repo>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Tree {
    pub(crate) root: PathBuf,
    pub(crate) repo: Segment,
    pub(crate) workspace: Segment,
}

impl Tree {
    /// The main checkout is the workspace `main`.
    const MAIN: &'static str = "main";
    /// What the directory holding a repo's other workspaces adds to its name.
    pub(crate) const SUFFIX: &'static str = "-ws";

    /// Finds the workspace `start` is in: the nearest directory above it with a `.jj`.
    pub(crate) fn find(start: &Path) -> Result<Self, Error> {
        let start = start.canonicalize().map_err(|source| Error::Io { path: start.to_path_buf(), source })?;
        let root = start
            .ancestors()
            .find(|dir| dir.join(".jj").is_dir())
            .ok_or_else(|| Error::NoRepo { start: start.clone() })?;
        Self::locate(root)
    }

    /// Finds the repo and workspace names from where `root` is.
    pub(crate) fn locate(root: &Path) -> Result<Self, Error> {
        let root = root.canonicalize().map_err(|source| Error::Io { path: root.to_path_buf(), source })?;
        let name = |path: &Path| path.file_name().and_then(|name| name.to_str()).map(str::to_owned);
        let own = name(&root).ok_or_else(|| Error::Utf8 { path: root.clone() })?;
        let parent = root.parent().and_then(name);
        let (repo, workspace) = match parent.as_deref().and_then(|dir| dir.strip_suffix(Self::SUFFIX)) {
            Some(repo) => (repo.to_owned(), own),
            None => (own, Self::MAIN.to_owned()),
        };
        let word = |text: String| text.parse::<Segment>().map_err(|source| Error::Name { path: root.clone(), source });
        Ok(Self { repo: word(repo)?, workspace: word(workspace)?, root: root.clone() })
    }

    /// Where the workspaces of the repo live: `<parent>/<repo>-ws`.
    pub(crate) fn container(&self) -> PathBuf {
        let own = self.root.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        match self.root.parent() {
            Some(parent)
                if own == self.workspace.as_str() && parent.ends_with(format!("{}{}", self.repo, Self::SUFFIX)) =>
            {
                parent.to_path_buf()
            }
            Some(parent) => parent.join(format!("{}{}", self.repo, Self::SUFFIX)),
            None => self.root.join(format!("{}{}", self.repo, Self::SUFFIX)),
        }
    }

    /// The tree's path on `host`: the workspace itself on this machine, a directory below the
    /// host's `dir` on another.
    pub(crate) fn on(&self, host: &Host) -> String {
        match host.via {
            Via::Local => self.root.to_string_lossy().into_owned(),
            Via::Ssh(_) => host.dir.join(&[self.repo.as_str(), self.workspace.as_str()]),
        }
    }

    /// The host's lock for syncing this workspace.
    pub(crate) fn lock(&self, host: &Host) -> String {
        host.dir.join(&["locks", &format!("{}.lock", self.workspace)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workspace_beside_the_main_checkout_is_named_by_its_directories() {
        let base = std::env::temp_dir().join(format!("farm-tree-{}", std::process::id()));
        let ws = base.join("uses.ink-ws").join("farm");
        let main = base.join("uses.ink");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::create_dir_all(&main).unwrap();
        let tree = Tree::locate(&ws).unwrap();
        assert_eq!((tree.repo.as_str(), tree.workspace.as_str()), ("uses.ink", "farm"));
        assert!(tree.container().ends_with("uses.ink-ws"));
        let tree = Tree::locate(&main).unwrap();
        assert_eq!((tree.repo.as_str(), tree.workspace.as_str()), ("uses.ink", "main"));
        assert!(tree.container().ends_with("uses.ink-ws"));
        let host = Host { via: Via::Ssh("x".parse().unwrap()), ..Host::local() };
        assert_eq!(tree.on(&host), "farm/uses.ink/main");
        assert_eq!(tree.lock(&host), "farm/locks/main.lock");
        assert_eq!(tree.on(&Host::local()), tree.root.to_string_lossy());
        let inside = ws.join(".jj");
        std::fs::create_dir_all(inside.join("repo")).unwrap();
        std::fs::create_dir_all(ws.join("a").join("b")).unwrap();
        assert_eq!(Tree::find(&ws.join("a").join("b")).unwrap().root, ws.canonicalize().unwrap());
        assert!(matches!(Tree::find(&main), Err(Error::NoRepo { .. })));
        std::fs::remove_dir_all(&base).unwrap();
    }
}
