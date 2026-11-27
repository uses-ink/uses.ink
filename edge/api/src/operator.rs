//! The operator's keys: `<data>/operator-keys`, an authorized-keys file (decision 0068, section
//! 10).
//!
//! One line per key: `ed25519-v1 <base64url public key> <label>`. Whoever can write the data
//! folder is the operator, the same trust the retired owner-token file had. Nothing secret is in
//! the file, so a backup of `<data>` leaks no credential, and the server never holds a private
//! key. The file is the single place: the API does not change it.

use std::error::Error;
use std::fmt;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::SystemTime;

use control::keys::{Label, LabelError};
use sig::{KeyType, PublicKey, Undecodable};

/// A key of the operator's file.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct OperatorKey {
    /// The type.
    pub key_type: KeyType,
    /// The public key.
    pub public: PublicKey,
    /// The label on its line.
    pub label: Label,
}

impl fmt::Display for OperatorKey {
    /// The key as its line is written.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{} {} {}", self.key_type, self.public, self.label)
    }
}

/// A line of the file that is not a key.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unreadable {
    /// The file could not be read.
    Read {
        /// The file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// A line is not `<type> <key> <label>`.
    Shape {
        /// The line, counted from 1.
        line: usize,
    },
    /// The key type is not one this server accepts.
    Type {
        /// The line.
        line: usize,
        /// Why.
        source: sig::UnknownKeyType,
    },
    /// The public key is not 43 base64url characters.
    Key {
        /// The line.
        line: usize,
        /// Why.
        source: Undecodable,
    },
    /// The label is no label.
    Label {
        /// The line.
        line: usize,
        /// Why.
        source: LabelError,
    },
}

impl fmt::Display for Unreadable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, .. } => write!(out, "{} could not be read", path.display()),
            Self::Shape { line } => write!(out, "line {line} of the operator keys is not `<type> <key> <label>`"),
            Self::Type { line, .. } => write!(out, "line {line} of the operator keys has a key type this server does not accept"),
            Self::Key { line, .. } => write!(out, "line {line} of the operator keys has a public key that is not valid"),
            Self::Label { line, .. } => write!(out, "line {line} of the operator keys has no usable label"),
        }
    }
}

impl Error for Unreadable {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::Type { source, .. } => Some(source),
            Self::Key { source, .. } => Some(source),
            Self::Label { source, .. } => Some(source),
            Self::Shape { .. } => None,
        }
    }
}

impl OperatorKey {
    /// The keys of a file's `text`: blank lines and `#` comments are skipped.
    pub fn parse(text: &str) -> Result<Vec<Self>, Unreadable> {
        let mut keys = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line_number = index + 1;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let mut parts = trimmed.splitn(3, ' ');
            let (Some(kind), Some(public), Some(label)) = (parts.next(), parts.next(), parts.next()) else {
                return Err(Unreadable::Shape { line: line_number });
            };
            keys.push(Self {
                key_type: kind.parse().map_err(|source| Unreadable::Type { line: line_number, source })?,
                public: public.parse().map_err(|source| Unreadable::Key { line: line_number, source })?,
                label: Label::parse(label.trim()).map_err(|source| Unreadable::Label { line: line_number, source })?,
            });
        }
        Ok(keys)
    }
}

/// Where the operator's keys come from. Futures are not needed: a file is read in place.
pub trait Operators {
    /// Why the keys could not be read.
    type Error: Error + 'static;

    /// Every key of the operator, as the source has them now.
    fn keys(&self) -> Result<Vec<OperatorKey>, Self::Error>;
}

/// What the file looked like when it was last read: its stamp and its keys.
#[derive(Debug)]
struct Cached {
    stamp: (SystemTime, u64),
    keys: Vec<OperatorKey>,
}

/// The `operator-keys` file. It is read per request, and re-read only when its modification
/// time or size changes (a `stat` a request is the cost), so a line added or deleted takes
/// effect on the next request. A file that does not exist holds no keys.
#[derive(Debug)]
pub struct OperatorFile {
    path: PathBuf,
    cache: Mutex<Option<Cached>>,
}

impl OperatorFile {
    /// The file at `path`.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), cache: Mutex::new(None) }
    }

    /// The file's path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn io(&self, source: std::io::Error) -> Unreadable {
        Unreadable::Read { path: self.path.clone(), source }
    }

    /// Adds `key` as a line, unless the file already has it. Whether it was added.
    pub fn add(&self, key: &OperatorKey) -> Result<bool, Unreadable> {
        let held = self.keys()?;
        if held.iter().any(|known| known.public == key.public) {
            return Ok(false);
        }
        let mut file = std::fs::OpenOptions::new().append(true).create(true).open(&self.path).map_err(|e| self.io(e))?;
        let separator = if std::fs::read_to_string(&self.path).is_ok_and(|text| !text.is_empty() && !text.ends_with('\n')) {
            "\n"
        } else {
            ""
        };
        writeln!(file, "{separator}{key}").map_err(|e| self.io(e))?;
        Ok(true)
    }
}

impl Operators for OperatorFile {
    type Error = Unreadable;

    fn keys(&self) -> Result<Vec<OperatorKey>, Unreadable> {
        let stamp = match std::fs::metadata(&self.path) {
            Ok(meta) => (meta.modified().map_err(|e| self.io(e))?, meta.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(self.io(error)),
        };
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(held) = cache.as_ref().filter(|held| held.stamp == stamp) {
            return Ok(held.keys.clone());
        }
        let text = std::fs::read_to_string(&self.path).map_err(|e| self.io(e))?;
        let keys = OperatorKey::parse(&text)?;
        *cache = Some(Cached { stamp, keys: keys.clone() });
        Ok(keys)
    }
}

/// No operator keys at all, for a server that has no `<data>` folder.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoOperators;

impl Operators for NoOperators {
    type Error = std::convert::Infallible;

    fn keys(&self) -> Result<Vec<OperatorKey>, Self::Error> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn line() -> String {
        format!("ed25519-v1 {} laptop at home", PublicKey::new([4; 32]))
    }

    #[test]
    fn lines_are_a_type_a_key_and_a_label_and_comments_are_skipped() {
        let keys = OperatorKey::parse(&format!("# the operator\n\n{}\n", line())).unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].label.as_str(), "laptop at home");
        assert_eq!(keys[0].to_string(), line());
        assert!(matches!(OperatorKey::parse("ed25519-v1"), Err(Unreadable::Shape { line: 1 })));
        assert!(matches!(OperatorKey::parse("rsa AAAA x"), Err(Unreadable::Type { line: 1, .. })));
        assert!(matches!(OperatorKey::parse("ed25519-v1 AAAA x"), Err(Unreadable::Key { line: 1, .. })));
    }

    #[test]
    fn the_file_is_read_again_when_it_changes_and_a_missing_file_holds_nothing() {
        let folder = tempfile::tempdir().unwrap();
        let file = OperatorFile::at(folder.path().join("operator-keys"));
        assert!(file.keys().unwrap().is_empty());
        let key = OperatorKey::parse(&line()).unwrap().remove(0);
        assert!(file.add(&key).unwrap());
        assert!(!file.add(&key).unwrap());
        assert_eq!(file.keys().unwrap(), [key]);
        std::fs::write(file.path(), "").unwrap();
        assert!(file.keys().unwrap().is_empty());
    }
}
