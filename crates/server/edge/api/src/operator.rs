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
use std::sync::{Arc, Mutex, PoisonError};

use control::keys::{Label, LabelError};
use logging::Field as _;
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
            Self::Type { line, .. } => {
                write!(out, "line {line} of the operator keys has a key type this server does not accept")
            }
            Self::Key { line, .. } => {
                write!(out, "line {line} of the operator keys has a public key that is not valid")
            }
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
    /// The keys of a file's `text`: blank lines and `#` comments are skipped, and the first line
    /// that is not a key is the error. [`OperatorFile`] reads with [`Roll::read`] instead, which
    /// skips such a line and says so.
    pub fn parse(text: &str) -> Result<Vec<Self>, Unreadable> {
        let mut keys = Vec::new();
        for (index, line) in text.lines().enumerate() {
            if let Some(key) = Self::line(index + 1, line) {
                keys.push(key?);
            }
        }
        Ok(keys)
    }

    /// The key a line holds, or none for a blank line and a `#` comment.
    fn line(number: usize, text: &str) -> Option<Result<Self, Unreadable>> {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }
        let mut parts = trimmed.splitn(3, ' ');
        let (Some(kind), Some(public), Some(label)) = (parts.next(), parts.next(), parts.next()) else {
            return Some(Err(Unreadable::Shape { line: number }));
        };
        Some(Self::fields(number, (kind, public, label)))
    }

    /// The key whose three fields a line gave.
    fn fields(line: usize, (kind, public, label): (&str, &str, &str)) -> Result<Self, Unreadable> {
        Ok(Self {
            key_type: kind.parse().map_err(|source| Unreadable::Type { line, source })?,
            public: public.parse().map_err(|source| Unreadable::Key { line, source })?,
            label: Label::parse(label.trim()).map_err(|source| Unreadable::Label { line, source })?,
        })
    }
}

/// What a file of operator keys holds: the keys of its lines that are keys, and what is wrong
/// with the others. One bad line costs the operator that line, not every signed request.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct Roll {
    /// The keys, in the order of the file.
    pub keys: Vec<OperatorKey>,
    /// The lines that are not keys, each with its line number and cause.
    pub skipped: Vec<Unreadable>,
}

impl Roll {
    /// Reads `text` line by line, keeping a bad line's error beside the keys.
    pub fn read(text: &str) -> Self {
        let mut roll = Self::default();
        for (index, line) in text.lines().enumerate() {
            match OperatorKey::line(index + 1, line) {
                Some(Ok(key)) => roll.keys.push(key),
                Some(Err(error)) => roll.skipped.push(error),
                None => {}
            }
        }
        roll
    }
}

/// Where the operator's keys come from. Futures are not needed: a file is read in place.
pub trait Operators {
    /// Why the keys could not be read.
    type Error: Error + 'static;

    /// Every key of the operator, as the source has them now.
    fn keys(&self) -> Result<Vec<OperatorKey>, Self::Error>;
}

/// What the file held when it was last read: the digest of its bytes, its keys and its bad lines.
#[derive(Debug)]
struct Cached {
    digest: blake3::Hash,
    keys: Vec<OperatorKey>,
    skipped: Arc<[Unreadable]>,
}

/// The `operator-keys` file. It is read per request and parsed only when the digest of its bytes
/// changes, so a line added or deleted takes effect on the next request, whatever the file's
/// modification time and size say (a tool may keep both). A file that does not exist holds no
/// keys. A line that is not a key is skipped, logged once when the file is parsed, and listed by
/// [`OperatorFile::skipped`]; the other keys still sign.
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

    /// The lines of the file that were not keys when it was last parsed.
    pub fn skipped(&self) -> Arc<[Unreadable]> {
        let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        cache.as_ref().map_or_else(|| Arc::from([]), |held| Arc::clone(&held.skipped))
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
        let mut file =
            std::fs::OpenOptions::new().append(true).create(true).open(&self.path).map_err(|e| self.io(e))?;
        let separator =
            if std::fs::read_to_string(&self.path).is_ok_and(|text| !text.is_empty() && !text.ends_with('\n')) {
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
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                *self.cache.lock().unwrap_or_else(PoisonError::into_inner) = None;
                return Ok(Vec::new());
            }
            Err(error) => return Err(self.io(error)),
        };
        let digest = blake3::hash(text.as_bytes());
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(held) = cache.as_ref().filter(|held| held.digest == digest) {
            return Ok(held.keys.clone());
        }
        let Roll { keys, skipped } = Roll::read(&text);
        for line in &skipped {
            logging::warn!(name: logging::event::OPERATOR_SKIPPED, error = logging::Failure(line).field());
        }
        *cache = Some(Cached { digest, keys: keys.clone(), skipped: skipped.into() });
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
        assert_eq!(file.keys().unwrap(), Vec::new());
        let key = OperatorKey::parse(&line()).unwrap().remove(0);
        assert!(file.add(&key).unwrap());
        assert!(!file.add(&key).unwrap());
        assert_eq!(file.keys().unwrap(), [key]);
        std::fs::write(file.path(), "").unwrap();
        assert_eq!(file.keys().unwrap(), Vec::new());
    }

    #[test]
    fn a_rewrite_of_the_same_size_under_the_same_time_is_read_again() {
        let folder = tempfile::tempdir().unwrap();
        let file = OperatorFile::at(folder.path().join("operator-keys"));
        let (first, second) = (line(), line().replace("laptop", "tablet"));
        std::fs::write(file.path(), &first).unwrap();
        let stamped = std::fs::metadata(file.path()).unwrap().modified().unwrap();
        assert_eq!(file.keys().unwrap()[0].label.as_str(), "laptop at home");
        std::fs::write(file.path(), &second).unwrap();
        std::fs::File::options().write(true).open(file.path()).unwrap().set_modified(stamped).unwrap();
        assert_eq!(first.len(), second.len());
        assert_eq!(file.keys().unwrap()[0].label.as_str(), "tablet at home");
    }

    #[test]
    fn a_bad_line_is_skipped_and_listed_and_the_other_keys_stay() {
        let folder = tempfile::tempdir().unwrap();
        let file = OperatorFile::at(folder.path().join("operator-keys"));
        std::fs::write(file.path(), format!("{}\nnonsense\nrsa AAAA x\n{}\n", line(), line())).unwrap();
        assert_eq!(file.keys().unwrap().len(), 2);
        let skipped = file.skipped();
        assert!(matches!(skipped[0], Unreadable::Shape { line: 2 }));
        assert!(matches!(skipped[1], Unreadable::Type { line: 3, .. }));
        assert_eq!(skipped.len(), 2);
        assert!(matches!(OperatorKey::parse("nonsense"), Err(Unreadable::Shape { line: 1 })));
    }
}
