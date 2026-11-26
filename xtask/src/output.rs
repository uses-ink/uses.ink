//! Where generated files go: written, or compared with what is on disk.

use std::path::{Path, PathBuf};

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Write each file.
    Write,
    /// Compare each file with the one on disk, writing nothing.
    Check,
}

/// The files a task produced, by path relative to the root.
#[derive(Debug)]
pub(crate) struct Output {
    root: PathBuf,
    mode: Mode,
    written: Vec<PathBuf>,
    stale: Vec<PathBuf>,
}

impl Output {
    pub(crate) fn new(root: PathBuf, mode: Mode) -> Self {
        Self { root, mode, written: Vec::new(), stale: Vec::new() }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// Reads a source file under the root.
    pub(crate) fn read(&self, path: impl AsRef<Path>) -> Result<String, Error> {
        let path = self.root.join(path);
        std::fs::read_to_string(&path).map_err(|source| Error::Read { path, source })
    }

    /// Puts `bytes` at `path` under the root.
    pub(crate) fn put(&mut self, path: impl AsRef<Path>, bytes: &[u8]) -> Result<(), Error> {
        let relative = path.as_ref().to_path_buf();
        let path = self.root.join(&relative);
        let current = std::fs::read(&path).ok();
        match self.mode {
            Mode::Check => {
                if current.as_deref() != Some(bytes) {
                    self.stale.push(relative);
                }
            }
            Mode::Write => {
                if current.as_deref() != Some(bytes) {
                    std::fs::write(&path, bytes).map_err(|source| Error::Write { path, source })?;
                    self.written.push(relative);
                }
            }
        }
        Ok(())
    }

    /// Fails when a checked file differs from what the generator writes.
    pub(crate) fn fresh(&self) -> Result<(), Error> {
        if self.stale.is_empty() { Ok(()) } else { Err(Error::Stale(self.stale.clone())) }
    }

    /// What happened, a line each.
    pub(crate) fn report(&self) -> Vec<String> {
        match self.mode {
            Mode::Check => vec!["check: layout fine, generated files fresh".to_owned()],
            Mode::Write if self.written.is_empty() => vec!["nothing changed".to_owned()],
            Mode::Write => self.written.iter().map(|p| format!("wrote {}", p.display())).collect(),
        }
    }
}
