//! A hash of a directory's files, stable across runs and machines: FNV-1a over each file's path
//! below the directory and its bytes, in path order. `build.rs` includes this file to hash the
//! source a binary is built from; `farm status` hashes the checkout again to see whether the
//! binary is behind it.

use std::fmt;
use std::io;
use std::num::ParseIntError;
use std::path::{Path, PathBuf};
use std::str::FromStr;

/// The hash of a directory's files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Digest(u64);

impl Digest {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    /// Hashes every file below `dir`.
    pub(crate) fn of(dir: &Path) -> io::Result<Self> {
        let mut files = Vec::new();
        Self::collect(dir, &mut files)?;
        files.sort();
        let mut hash = Self::OFFSET;
        for file in files {
            let name = file.strip_prefix(dir).unwrap_or(&file).to_string_lossy().into_owned();
            hash = Self::fold(hash, name.as_bytes());
            hash = Self::fold(hash, &[0]);
            hash = Self::fold(hash, &std::fs::read(&file)?);
            hash = Self::fold(hash, &[0]);
        }
        Ok(Self(hash))
    }

    fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                Self::collect(&path, files)?;
            } else {
                files.push(path);
            }
        }
        Ok(())
    }

    fn fold(hash: u64, bytes: &[u8]) -> u64 {
        bytes.iter().fold(hash, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(Self::PRIME))
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{:016x}", self.0)
    }
}

impl FromStr for Digest {
    type Err = ParseIntError;

    fn from_str(given: &str) -> Result<Self, Self::Err> {
        u64::from_str_radix(given, 16).map(Self)
    }
}
