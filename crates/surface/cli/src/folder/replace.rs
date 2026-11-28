//! Files replaced together, none of them half written: every text goes to a file of its own beside
//! its target first, and only when all are written is each renamed over its target. A write
//! that fails (a full disk, a folder that cannot be written) leaves every target as it was; the
//! renames that follow are within one folder each and are the only step a failure can split.
//! A temporary file a crash left (`.name.ink-new`) is never overwritten: it stops the next
//! replacement with a typed error naming it.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::output::error::{Error, FolderError};

/// Texts to put in place of files, all or none.
#[derive(Debug, Default)]
pub(crate) struct Replacing {
    files: Vec<(PathBuf, String)>,
}

/// A text written beside its target and not yet renamed over it.
#[derive(Debug)]
struct Staged {
    temp: PathBuf,
    target: PathBuf,
}

impl Replacing {
    /// What a temporary file's name ends with.
    const SUFFIX: &'static str = "ink-new";

    /// Replaces `path` with `text`, when [`Replacing::commit`] runs.
    pub(crate) fn push(&mut self, path: PathBuf, text: String) {
        self.files.push((path, text));
    }

    /// Writes every text, then puts each in place; the number of files replaced.
    pub(crate) fn commit(self) -> Result<usize, Error> {
        let mut staged: Vec<Staged> = Vec::with_capacity(self.files.len());
        for (path, text) in &self.files {
            match Staged::write(path, text) {
                Ok(one) => staged.push(one),
                Err(error) => {
                    staged.iter().for_each(Staged::discard);
                    return Err(error);
                }
            }
        }
        for (done, one) in staged.iter().enumerate() {
            if let Err(source) = std::fs::rename(&one.temp, &one.target) {
                staged.iter().skip(done).for_each(Staged::discard);
                return Err(FolderError::Write { path: one.target.clone(), source }.into());
            }
        }
        Ok(staged.len())
    }
}

impl Staged {
    /// `text` in a new file beside `path`, with `path`'s permissions when it exists. A symbolic
    /// link is followed, so the file it names is the one replaced.
    fn write(path: &Path, text: &str) -> Result<Self, Error> {
        let write = |source| Error::from(FolderError::Write { path: path.to_path_buf(), source });
        let target = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let name = target.file_name().map_or_default(|name| name.to_string_lossy().into_owned());
        let temp = target.with_file_name(format!(".{name}.{}", Replacing::SUFFIX));
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(write)?;
        let staged = Self { temp, target };
        let written = file.write_all(text.as_bytes()).and_then(|()| file.sync_all()).and_then(|()| {
            match std::fs::metadata(&staged.target) {
                Ok(metadata) => file.set_permissions(metadata.permissions()),
                Err(_) => Ok(()),
            }
        });
        written.map_err(|source| {
            staged.discard();
            write(source)
        })?;
        Ok(staged)
    }

    /// Removes the temporary file, which nothing else is left to do about when it cannot be.
    fn discard(&self) {
        std::fs::remove_file(&self.temp).ok();
    }
}
