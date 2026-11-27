//! The key and its file.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use chacha20poly1305::aead::OsRng;
use chacha20poly1305::aead::rand_core::RngCore;
use trace::Field as _;
use zeroize::Zeroizing;

use crate::Error;

/// How a key came to be in memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Origin {
    /// Read from its file.
    Read,
    /// Made now, and kept in its file.
    Made,
}

/// The 32-byte key, zeroed when dropped. It has no `Debug`, no `Clone` and no accessor outside
/// this crate.
pub(crate) struct Key(Zeroizing<[u8; Self::LEN]>);

impl Key {
    pub(crate) const LEN: usize = 32;

    /// A key from the operating system's randomness.
    pub(crate) fn generate() -> Result<Self, Error> {
        let mut bytes = Zeroizing::new([0_u8; Self::LEN]);
        OsRng.try_fill_bytes(bytes.as_mut()).map_err(|source| Error::Random { source })?;
        Ok(Self(bytes))
    }

    pub(crate) fn bytes(&self) -> &[u8; Self::LEN] {
        &self.0
    }

    /// The key in `path`, made there if the file is missing. An existing file must be a plain
    /// file, 32 bytes, readable by its owner only.
    ///
    /// A new key is written whole to a file of its own beside `path`, synced, and only then
    /// linked to `path`, so `path` is never seen holding part of a key, and a crash in between
    /// leaves a stray file and no key. A link refuses to replace a file, so of two processes that
    /// start together the first key stays and the other reads it.
    pub(crate) fn open(path: &Path) -> Result<(Self, Origin), Error> {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_file() => Self::read(path, &meta).map(|key| (key, Origin::Read)),
            Ok(_) => Err(Error::NotFile { path: path.to_path_buf() }),
            Err(source) if source.kind() == ErrorKind::NotFound => Self::create(path),
            Err(source) => Err(Error::Key { path: path.to_path_buf(), source }),
        }
    }

    /// A new key, kept in `path`; the one another process kept there first, if one did.
    fn create(path: &Path) -> Result<(Self, Origin), Error> {
        let made = Self::generate()?;
        let folder = path.parent().filter(|folder| !folder.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
        let mut tag = [0_u8; 8];
        OsRng.try_fill_bytes(&mut tag).map_err(|source| Error::Random { source })?;
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let temp = folder.join(format!(".{name}.{:016x}.new", u64::from_le_bytes(tag)));
        let key = |at: &Path| {
            let at = at.to_path_buf();
            move |source: std::io::Error| Error::Key { path: at.clone(), source }
        };
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        Self::private(&mut options);
        let written = options
            .open(&temp)
            .and_then(|mut file| file.write_all(made.bytes()).and_then(|()| file.sync_all()))
            .map_err(key(&temp));
        let linked = written.and_then(|()| match fs::hard_link(&temp, path) {
            Ok(()) => Ok(true),
            Err(source) if source.kind() == ErrorKind::AlreadyExists => Ok(false),
            Err(source) => Err(key(path)(source)),
        });
        let gone = fs::remove_file(&temp).map_err(key(&temp));
        match linked {
            Ok(true) => {
                gone?;
                Self::sync(folder)?;
                Ok((made, Origin::Made))
            }
            Ok(false) => {
                gone?;
                // the winner linked its key after syncing it, so the file is whole
                Self::open(path)
            }
            Err(error) => {
                // the first failure is the one returned; a temporary that stays is said besides
                if let Err(stays) = gone {
                    trace::warn!(name: trace::event::KEY_REFUSED, kind = "leftover", error = trace::Failure(&stays).field());
                }
                Err(error)
            }
        }
    }

    /// Makes the new name in `folder` durable.
    #[cfg(unix)]
    fn sync(folder: &Path) -> Result<(), Error> {
        let at: PathBuf = folder.to_path_buf();
        fs::File::open(&at).and_then(|file| file.sync_all()).map_err(|source| Error::Key { path: at, source })
    }

    /// Windows has no way to sync a folder.
    #[cfg(not(unix))]
    fn sync(_folder: &Path) -> Result<(), Error> {
        Ok(())
    }

    fn read(path: &Path, meta: &fs::Metadata) -> Result<Self, Error> {
        Self::check(path, meta)?;
        if meta.len() != Self::LEN as u64 {
            return Err(Error::Length { path: path.to_path_buf(), found: meta.len() });
        }
        let key = |source| Error::Key { path: path.to_path_buf(), source };
        let mut bytes = Zeroizing::new([0_u8; Self::LEN]);
        fs::File::open(path).map_err(key)?.read_exact(bytes.as_mut()).map_err(key)?;
        Ok(Self(bytes))
    }

    /// A file made for the key is readable by its owner only.
    #[cfg(unix)]
    fn private(options: &mut OpenOptions) {
        std::os::unix::fs::OpenOptionsExt::mode(options, 0o600);
    }

    /// Windows has no mode bits: the file inherits its folder's access list.
    #[cfg(not(unix))]
    fn private(_options: &mut OpenOptions) {}

    /// The permission bits of the group and of everyone else: none may be set.
    #[cfg(unix)]
    const GROUP_AND_OTHER: u32 = 0o077;

    #[cfg(unix)]
    fn check(path: &Path, meta: &fs::Metadata) -> Result<(), Error> {
        let mode = std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o777;
        if mode & Self::GROUP_AND_OTHER == 0 {
            Ok(())
        } else {
            Err(Error::Permissions { path: path.to_path_buf(), mode })
        }
    }

    #[cfg(not(unix))]
    fn check(_path: &Path, _meta: &fs::Metadata) -> Result<(), Error> {
        Ok(())
    }
}
