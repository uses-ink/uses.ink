//! Writes the control plane's migrations as the files `wrangler d1 migrations apply` reads,
//! `migrations/NNNN_name.sql`, rendered by the port's one renderer (decision 0098). The files are
//! generated and never committed: `scripts <folder>` writes them below `<folder>`.

use std::fmt;
use std::path::PathBuf;

use db::{StepError, Unrenderable};
use uses_ink_edge_worker::Migrations;

/// What stopped the run.
#[derive(Debug)]
enum Fault {
    /// No folder was given.
    Usage,
    /// The migrations could not be listed.
    Steps(StepError),
    /// A statement could not be rendered.
    Render(Unrenderable),
    /// A file or folder could not be written.
    Write { path: PathBuf, source: std::io::Error },
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage => out.write_str("usage: scripts <folder>"),
            Self::Steps(_) => out.write_str("the migrations could not be listed"),
            Self::Render(_) => out.write_str("a statement cannot be written as text"),
            Self::Write { path, .. } => write!(out, "cannot write {}", path.display()),
        }
    }
}

impl std::error::Error for Fault {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Usage => None,
            Self::Steps(source) => Some(source),
            Self::Render(source) => Some(source),
            Self::Write { source, .. } => Some(source),
        }
    }
}

fn main() -> Result<(), Fault> {
    let folder = std::env::args_os().nth(1).map(PathBuf::from).ok_or(Fault::Usage)?;
    let scripts = Migrations::steps().map_err(Fault::Steps)?.scripts().map_err(Fault::Render)?;
    let out = folder.join("migrations");
    std::fs::create_dir_all(&out).map_err(|source| Fault::Write { path: out.clone(), source })?;
    for script in scripts {
        let path = out.join(&script.name);
        std::fs::write(&path, &script.text).map_err(|source| Fault::Write { path, source })?;
    }
    Ok(())
}
