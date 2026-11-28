//! A file opened once and read through that one handle, so what was checked is what is read.

use std::fs::{File, Metadata};
use std::io::Read as _;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::Path;

/// `O_NONBLOCK`: opening a pipe for reading returns at once instead of waiting for a writer.
// HACK(libc becomes a dependency of this crate): the flag's value, which std does not export.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
const NONBLOCK: i32 = 0x0004;
#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
)))]
const NONBLOCK: i32 = 0o4000;

/// Which file a path names: its device and inode, the same for every link to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Identity {
    device: u64,
    inode: u64,
}

impl From<&Metadata> for Identity {
    fn from(metadata: &Metadata) -> Self {
        Self { device: metadata.dev(), inode: metadata.ino() }
    }
}

/// An open file and what the handle itself says it is.
#[derive(Debug)]
pub(crate) struct Handle {
    file: File,
    metadata: Metadata,
}

impl Handle {
    /// Opens `path` for reading without waiting on a pipe, then asks the handle what it opened.
    pub(crate) fn open(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new().read(true).custom_flags(NONBLOCK).open(path)?;
        let metadata = file.metadata()?;
        Ok(Self { file, metadata })
    }

    /// What the handle says: its type, length and identity.
    pub(crate) fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    /// At most `limit` bytes, so a file that grows while it is read costs no more than that.
    pub(crate) fn bytes(self, limit: u64) -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.file.take(limit).read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}
