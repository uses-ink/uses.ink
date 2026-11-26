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
    removed: Vec<PathBuf>,
    stale: Vec<PathBuf>,
    /// Every path put, so a claimed directory's leftovers can be found.
    put: Vec<PathBuf>,
}

impl Output {
    pub(crate) fn new(root: PathBuf, mode: Mode) -> Self {
        Self { root, mode, written: Vec::new(), removed: Vec::new(), stale: Vec::new(), put: Vec::new() }
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
        self.put.push(relative.clone());
        let current = match std::fs::read(&path) {
            Ok(current) => Some(current),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(source) => return Err(Error::Read { path, source }),
        };
        if current.as_deref() == Some(bytes) {
            return Ok(());
        }
        match self.mode {
            Mode::Check => self.stale.push(relative),
            Mode::Write => {
                std::fs::write(&path, bytes).map_err(|source| Error::Write { path, source })?;
                self.written.push(relative);
            }
        }
        Ok(())
    }

    /// Claims every `.{extension}` file directly in `dir` as generated: one this run did not put
    /// is a leftover, removed when writing and stale when checking. Call after the puts.
    pub(crate) fn claim(&mut self, dir: impl AsRef<Path>, extension: &str) -> Result<(), Error> {
        let dir = self.root.join(dir.as_ref());
        let entries = std::fs::read_dir(&dir).map_err(|source| Error::Read { path: dir.clone(), source })?;
        for entry in entries {
            let entry = entry.map_err(|source| Error::Read { path: dir.clone(), source })?;
            let path = entry.path();
            let relative = path.strip_prefix(&self.root).unwrap_or(&path).to_path_buf();
            if path.extension().is_none_or(|e| e != extension) || self.put.contains(&relative) {
                continue;
            }
            match self.mode {
                Mode::Check => self.stale.push(relative),
                Mode::Write => {
                    std::fs::remove_file(&path).map_err(|source| Error::Write { path, source })?;
                    self.removed.push(relative);
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
            Mode::Write if self.written.is_empty() && self.removed.is_empty() => vec!["nothing changed".to_owned()],
            Mode::Write => {
                let wrote = self.written.iter().map(|p| format!("wrote {}", p.display()));
                wrote.chain(self.removed.iter().map(|p| format!("removed {} (no longer drawn)", p.display()))).collect()
            }
        }
    }
}
