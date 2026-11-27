//! The key and its file.

use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use chacha20poly1305::aead::OsRng;
use chacha20poly1305::aead::rand_core::RngCore;
use zeroize::Zeroizing;

use crate::Error;

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
    pub(crate) fn open(path: &Path) -> Result<Self, Error> {
        let key = |source| Error::Key { path: path.to_path_buf(), source };
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_file() => Self::read(path, &meta),
            Ok(_) => Err(Error::NotFile { path: path.to_path_buf() }),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                let made = Self::generate()?;
                let mut options = OpenOptions::new();
                options.write(true).create_new(true);
                Self::private(&mut options);
                let mut file = options.open(path).map_err(key)?;
                file.write_all(made.bytes()).map_err(key)?;
                file.sync_all().map_err(key)?;
                Ok(made)
            }
            Err(source) => Err(key(source)),
        }
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

    #[cfg(unix)]
    fn check(path: &Path, meta: &fs::Metadata) -> Result<(), Error> {
        let mode = std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o777;
        if mode & 0o077 == 0 { Ok(()) } else { Err(Error::Permissions { path: path.to_path_buf(), mode }) }
    }

    #[cfg(not(unix))]
    fn check(_path: &Path, _meta: &fs::Metadata) -> Result<(), Error> {
        Ok(())
    }
}
