//! Where generated files go: written, or compared with what is on disk.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use proc_macro2::TokenStream;

use crate::Task;
use crate::error::Error;
use crate::process::Process;

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

    /// Puts generated Rust at `path`: printed by prettyplease, then rustfmt with the
    /// workspace's settings, so the file on disk is what `cargo fmt` would leave. The header
    /// says where it came from.
    pub(crate) fn put_rust(&mut self, path: &str, origin: Origin, code: TokenStream) -> Result<(), Error> {
        let file: syn::File = syn::parse2(code).map_err(|source| Error::Generated { path: path.into(), source })?;
        let text = format!("// {origin}\n\n{}", prettyplease::unparse(&file));
        let text = rustfmt(&self.root, path, &text)?;
        self.put(path, text.as_bytes())
    }

    /// Fails when a checked file differs from what the generator writes.
    pub(crate) fn fresh(&self) -> Result<(), Error> {
        if self.stale.is_empty() { Ok(()) } else { Err(Error::Stale(self.stale.clone())) }
    }

    /// What happened, a line each.
    pub(crate) fn report(&self) -> Vec<Line> {
        match self.mode {
            Mode::Check => vec![Line::Checked],
            Mode::Write if self.written.is_empty() && self.removed.is_empty() => vec![Line::Unchanged],
            Mode::Write => {
                let wrote = self.written.iter().cloned().map(Line::Wrote);
                wrote.chain(self.removed.iter().cloned().map(Line::Removed)).collect()
            }
        }
    }
}

/// Where a generated file comes from: the task that writes it and the file it is written from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Origin {
    pub(crate) task: Task,
    pub(crate) source: &'static str,
}

impl fmt::Display for Origin {
    /// The sentence a generated file starts with, inside a comment.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "Generated by `cargo xtask {}` from `{}`. Edit that file, not this one.", self.task, self.source)
    }
}

/// One line of a task's report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Line {
    /// A generated file written, by path relative to the root.
    Wrote(PathBuf),
    /// A file in a claimed directory removed because nothing generates it any more.
    Removed(PathBuf),
    /// Every generated file already held what the generator writes.
    Unchanged,
    /// The layout and every generated file are as they should be.
    Checked,
}

impl fmt::Display for Line {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wrote(path) => write!(out, "wrote {}", path.display()),
            Self::Removed(path) => write!(out, "removed {} (no longer generated)", path.display()),
            Self::Unchanged => write!(out, "nothing changed"),
            Self::Checked => write!(out, "check: layout fine, generated files fresh"),
        }
    }
}

fn rustfmt(root: &Path, path: &str, text: &str) -> Result<String, Error> {
    let mut command = Command::new("rustfmt");
    command
        .args(["--edition", "2024", "--emit", "stdout", "--config-path"])
        .arg(root.join("rustfmt.toml"))
        .current_dir(root);
    Process::from(command).text(text.as_bytes()).map_err(|source| Error::Rustfmt { path: path.into(), source })
}
