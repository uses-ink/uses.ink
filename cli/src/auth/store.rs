//! Where a machine keeps its private key: a file of its own in the user's config folder.
//!
//! `~/.config/ink/keys/<server>.key`, in a folder only the user may enter (0700) and a file only
//! the user may read (0600), made with `create_new` so a key is never overwritten by accident.
//! The key text is never in the config file, never in an argument and never printed. An OS
//! keystore (Secret Service, Keychain) is not used yet: a file with those modes is what `ssh`
//! relies on, and a keystore adapter would sit behind [`Keystore`] without changing a caller.

use std::fmt;
use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
use std::path::PathBuf;

use sig::{KeyError, Seed};

use crate::settings::{Environment, Preferences};

/// A key file that could not be kept or read.
#[derive(Debug)]
#[non_exhaustive]
pub enum StoreError {
    /// There is no config folder: neither `XDG_CONFIG_HOME` nor `HOME` is set.
    NoHome,
    /// A file or folder could not be made, read or removed.
    Io {
        /// Where.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// A key file holds no key.
    Unreadable {
        /// Where.
        path: PathBuf,
        /// Why.
        source: KeyError,
    },
    /// A key is already kept for this server.
    Held {
        /// Where.
        path: PathBuf,
    },
}

impl fmt::Display for StoreError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHome => {
                out.write_str("there is no config folder to keep a key in: set `HOME` or `XDG_CONFIG_HOME`")
            }
            Self::Io { path, .. } => write!(out, "{} could not be made, read or removed", path.display()),
            Self::Unreadable { path, .. } => write!(out, "{} holds no key", path.display()),
            Self::Held { path } => {
                write!(out, "{} already holds a key for this server: run `ink logout` first", path.display())
            }
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Unreadable { source, .. } => Some(source),
            Self::NoHome | Self::Held { .. } => None,
        }
    }
}

/// The folder of key files and what is kept in it.
#[derive(Clone, Debug)]
pub(crate) struct Keystore {
    folder: PathBuf,
}

impl Keystore {
    /// The store of the user's config folder.
    pub(crate) fn of(env: &impl Environment) -> Result<Self, StoreError> {
        let config = Preferences::path(env).ok_or(StoreError::NoHome)?;
        let base = config.parent().ok_or(StoreError::NoHome)?;
        Ok(Self { folder: base.join("keys") })
    }

    /// The file for the server whose authority is `authority`: only characters a file name keeps.
    pub(crate) fn file(&self, authority: &str) -> PathBuf {
        let name: String = authority
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' })
            .collect();
        self.folder.join(format!("{name}.key"))
    }

    /// The key kept for `authority`, when there is one.
    pub(crate) fn load(&self, authority: &str) -> Result<Option<Seed>, StoreError> {
        let path = self.file(authority);
        match std::fs::read_to_string(&path) {
            Ok(text) => text.trim().parse().map(Some).map_err(|source| StoreError::Unreadable { path, source }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(StoreError::Io { path, source }),
        }
    }

    /// Keeps `seed` for `authority`, in a file only the user can read. Never overwrites.
    pub(crate) fn save(&self, authority: &str, seed: &Seed) -> Result<PathBuf, StoreError> {
        let path = self.file(authority);
        let io = |source| StoreError::Io { path: path.clone(), source };
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&self.folder).map_err(io)?;
        let mut file = match std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Err(StoreError::Held { path }),
            Err(error) => return Err(io(error)),
        };
        writeln!(file, "{}", seed.text().reveal()).map_err(io)?;
        Ok(path)
    }

    /// Forgets the key kept for `authority`. Whether there was one.
    pub(crate) fn remove(&self, authority: &str) -> Result<bool, StoreError> {
        let path = self.file(authority);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(source) => Err(StoreError::Io { path, source }),
        }
    }
}

#[cfg(test)]
mod test {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    fn store() -> (tempfile::TempDir, Keystore) {
        let home = tempfile::tempdir().unwrap();
        let store = Keystore { folder: home.path().join("ink").join("keys") };
        (home, store)
    }

    #[test]
    fn a_key_is_kept_private_read_back_and_never_overwritten() {
        let (_home, store) = store();
        let seed = Seed::new([7; 32]);
        let path = store.save("api.localhost:8080", &seed).unwrap();
        assert_eq!(path.file_name().unwrap(), "api.localhost_8080.key");
        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!((mode(&path), mode(&store.folder)), (0o600, 0o700));
        let again = store.load("api.localhost:8080").unwrap().unwrap();
        assert_eq!(again.text().reveal(), seed.text().reveal());
        assert!(matches!(store.save("api.localhost:8080", &seed), Err(StoreError::Held { .. })));
        assert!(store.remove("api.localhost:8080").unwrap());
        assert!(store.load("api.localhost:8080").unwrap().is_none());
        assert!(!store.remove("api.localhost:8080").unwrap());
    }

    #[test]
    fn a_file_that_holds_no_key_says_which() {
        let (_home, store) = store();
        let path = store.file("x");
        std::fs::create_dir_all(&store.folder).unwrap();
        std::fs::write(&path, "not a key").unwrap();
        assert!(matches!(store.load("x"), Err(StoreError::Unreadable { .. })));
    }
}
