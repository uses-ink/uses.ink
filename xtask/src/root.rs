//! Where the workspace is: found from the directory the task runs in, never baked in at build
//! time, so a build cache copied from another checkout cannot make a task read that checkout.

use std::error::Error as StdError;
use std::fmt;
use std::path::PathBuf;

use crate::Error;

/// Why the workspace was not found.
#[derive(Debug)]
pub(crate) enum Lost {
    /// The current directory cannot be read.
    Directory(std::io::Error),
    /// No directory from the current one upward holds the workspace manifest.
    Rootless { start: PathBuf },
}

impl fmt::Display for Lost {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Directory(_) => write!(out, "cannot read the current directory"),
            Self::Rootless { start } => write!(out, "no workspace manifest at or above {}", start.display()),
        }
    }
}

impl StdError for Lost {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Directory(source) => Some(source),
            Self::Rootless { .. } => None,
        }
    }
}

/// Finds the workspace root.
pub(crate) struct Root;

impl Root {
    /// The nearest directory, from the current one upward, whose `Cargo.toml` has a `[workspace]`
    /// table.
    pub(crate) fn find() -> Result<PathBuf, Error> {
        let start = std::env::current_dir().map_err(|source| Error::Root(Lost::Directory(source)))?;
        for dir in start.ancestors() {
            let manifest = dir.join("Cargo.toml");
            match std::fs::read_to_string(&manifest) {
                Ok(text) if text.lines().any(|line| line.trim() == "[workspace]") => return Ok(dir.to_path_buf()),
                Ok(_) => {}
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
                Err(source) => return Err(Error::Read { path: manifest, source }),
            }
        }
        Err(Error::Root(Lost::Rootless { start }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_root_is_the_checkout_the_task_runs_in() {
        let found = Root::find().unwrap().canonicalize().unwrap();
        let built = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").canonicalize().unwrap();
        assert_eq!(found, built, "the tests run in the checkout they were built from");
    }
}
