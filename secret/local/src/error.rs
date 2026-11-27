//! What the local adapter fails with.

use std::fmt;
use std::path::PathBuf;

/// A failure of the local adapter.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The key file could not be read, created or synced.
    Key {
        /// The file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// The key file is not a plain file: a symlink or a folder.
    NotFile {
        /// The path.
        path: PathBuf,
    },
    /// The key file can be read by someone other than its owner.
    Permissions {
        /// The file.
        path: PathBuf,
        /// Its permission bits.
        mode: u32,
    },
    /// The key file is not 32 bytes.
    Length {
        /// The file.
        path: PathBuf,
        /// How long it is.
        found: u64,
    },
    /// The operating system gave no randomness.
    Random {
        /// What it said.
        source: rand_core::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key { path, .. } => write!(out, "the key file {} could not be read or made", path.display()),
            Self::NotFile { path } => write!(out, "the key file {} is not a plain file", path.display()),
            Self::Permissions { path, mode } => {
                write!(out, "the key file {} has mode {:03o}; only its owner may read it (0600)", path.display(), mode)
            }
            Self::Length { path, found } => write!(out, "the key file {} is {found} bytes, not 32", path.display()),
            Self::Random { .. } => out.write_str("the operating system gave no randomness"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Key { source, .. } => Some(source),
            Self::Random { source } => Some(source),
            Self::NotFile { .. } | Self::Permissions { .. } | Self::Length { .. } => None,
        }
    }
}
