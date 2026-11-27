//! The `git` capability a source plugin asks of its host (decisions 0036 and 0046): fetch a
//! repository into a cache the host manages, then read the fetched commit as a [`Snapshot`].
//!
//! A [`Request`] names a repository by its [`Locator`], what to fetch of it ([`Revision`],
//! [`Depth`]) and, for a private one, the name of the credential to fetch it with ([`Grant`]).
//! The name is all a plugin ever holds: the host keeps the secret and never lets it cross.
//! A fetch gives a [`Cloned`], a handle to what the host now holds; the snapshot is read from
//! that. Everything the host refuses is a typed [`Refused`] with its numbers, and a failure that
//! has no case of its own is [`Refused::Failed`], whose detail the host has already said.
//!
//! [`Git`] is the capability as a native trait, so a source's rule is written once and run
//! against the host's own implementation natively and against the imported one in a component.

use std::fmt;
use std::num::NonZeroU32;
use std::str::FromStr;

use crate::{Bytes, Diagnostic, Hash, Oid, Problem, Snapshot};

/// The kind of address a repository has: what its [`Locator`] starts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Scheme {
    /// A repository on the builder's own disk, `file:///path`.
    File,
    /// A repository on another machine, `https://host/path`.
    Https,
}

impl Scheme {
    /// How an address starts, before `://`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Https => "https",
        }
    }
}

impl fmt::Display for Scheme {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// Where a repository is: `file:///absolute/path` or `https://host/path`. Any other scheme, and
/// the scp-like `host:path` that git reads as ssh, is refused here, so no address reaches a
/// transport that was not meant to run.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Locator {
    scheme: Scheme,
    rest: String,
}

/// Why text is no [`Locator`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum LocatorError {
    /// The scheme is not one the builder fetches, or the address has none.
    Scheme {
        /// What stands before `://`, or the whole text when it has no `://`.
        written: String,
    },
    /// Nothing follows the scheme, or a `file` address that is not an absolute path.
    Path,
    /// A space or control character, at this byte offset.
    Character {
        /// The byte offset.
        at: usize,
    },
}

impl fmt::Display for LocatorError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scheme { written } => write!(out, "`{written}` is not a scheme of `file` or `https`"),
            Self::Path => out.write_str("the address names no repository path"),
            Self::Character { at } => write!(out, "byte {at} of the address is a space or control character"),
        }
    }
}

impl std::error::Error for LocatorError {}

impl Locator {
    /// The scheme.
    pub const fn scheme(&self) -> Scheme {
        self.scheme
    }
}

impl FromStr for Locator {
    type Err = LocatorError;

    fn from_str(text: &str) -> Result<Self, LocatorError> {
        let (written, rest) =
            text.split_once("://").ok_or_else(|| LocatorError::Scheme { written: text.to_owned() })?;
        let scheme = match written {
            "file" => Scheme::File,
            "https" => Scheme::Https,
            other => return Err(LocatorError::Scheme { written: other.to_owned() }),
        };
        if let Some(at) = rest.bytes().position(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control()) {
            return Err(LocatorError::Character { at: written.len() + 3 + at });
        }
        let named = match scheme {
            Scheme::File => rest.starts_with('/') && rest.len() > 1,
            Scheme::Https => !rest.is_empty() && !rest.starts_with('/'),
        };
        if !named {
            return Err(LocatorError::Path);
        }
        Ok(Self { scheme, rest: rest.to_owned() })
    }
}

impl fmt::Display for Locator {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}://{}", self.scheme, self.rest)
    }
}

/// A branch, tag or commit id of the repository, as `git rev-parse` reads it. It never starts
/// with `-` and holds no space or control character.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Revision(String);

/// Why text is no [`Revision`] or [`Grant`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum NameError {
    /// It is empty, or longer than a name may be.
    Length {
        /// How long it is.
        found: usize,
    },
    /// A character it may not hold, at this byte offset.
    Character {
        /// The byte offset.
        at: usize,
    },
}

impl fmt::Display for NameError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => write!(out, "the name is {found} bytes long, which is not allowed"),
            Self::Character { at } => write!(out, "byte {at} of the name is not allowed"),
        }
    }
}

impl std::error::Error for NameError {}

impl Revision {
    /// The longest a revision may be, in bytes.
    pub const MOST: usize = 256;

    /// The revision as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Revision {
    type Err = NameError;

    fn from_str(text: &str) -> Result<Self, NameError> {
        if text.is_empty() || text.len() > Self::MOST {
            return Err(NameError::Length { found: text.len() });
        }
        let forbidden = |byte: u8| byte.is_ascii_whitespace() || byte.is_ascii_control();
        match text.bytes().position(forbidden).or_else(|| text.starts_with('-').then_some(0)) {
            Some(at) => Err(NameError::Character { at }),
            None => Ok(Self(text.to_owned())),
        }
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// The name of a credential the host holds, such as `github`. A plugin names the credential it
/// wants used and never sees what it holds: 1 to 64 of `a` to `z`, `0` to `9`, `.`, `_` and `-`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Grant(String);

impl Grant {
    /// The longest a grant's name may be, in bytes.
    pub const MOST: usize = 64;

    /// The name as written.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Grant {
    type Err = NameError;

    fn from_str(text: &str) -> Result<Self, NameError> {
        if text.is_empty() || text.len() > Self::MOST {
            return Err(NameError::Length { found: text.len() });
        }
        let allowed = |byte: u8| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-');
        match text.bytes().position(|byte| !allowed(byte)) {
            Some(at) => Err(NameError::Character { at }),
            None => Ok(Self(text.to_owned())),
        }
    }
}

impl fmt::Display for Grant {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// How much history to fetch (decision 0036).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Depth {
    /// Every commit: a clone in full, which is the road to history. A shallow clone already in
    /// the cache is deepened to it.
    Full,
    /// Only this many commits from the tip: a snapshot first, or a bounded window. Dates of files
    /// older than the window are bounds.
    Commits(NonZeroU32),
}

/// One fetch.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Request {
    /// The repository.
    pub locator: Locator,
    /// The branch, tag or commit to read; the repository's own `HEAD` when none is named.
    pub revision: Option<Revision>,
    /// How much history.
    pub depth: Depth,
    /// The credential to fetch with, when the repository is private.
    pub grant: Option<Grant>,
}

impl Request {
    /// The repository's place in the host's cache: the hash of the address, revision and grant as
    /// written, so two revisions of one repository are two clones, and a clone fetched with one
    /// credential is never the one a request with another (or none) reads.
    pub fn key(&self) -> Hash {
        let revision = self.revision.as_ref().map_or_else(String::new, ToString::to_string);
        let grant = self.grant.as_ref().map_or_else(String::new, ToString::to_string);
        Hash::of(format!("{}\n{revision}\n{grant}", self.locator))
    }

    /// The whole history of the repository at `locator`, at its `HEAD`, with no credential.
    pub fn new(locator: Locator) -> Self {
        Self { locator, revision: None, depth: Depth::Full, grant: None }
    }
}

/// Whether a site's dates come from history.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum History {
    /// They do: the source fetches every commit, so each file's dates are the commits that made
    /// them.
    #[default]
    Wanted,
    /// They do not: one commit is enough, and every date is a bound.
    Unneeded,
}

/// What a source plugin is asked to read: a repository, and how it will be used. The plugin
/// decides what that takes to fetch.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Want {
    /// The repository.
    pub locator: Locator,
    /// The branch, tag or commit to read; the repository's `HEAD` when none is named.
    pub revision: Option<Revision>,
    /// Whether dates come from history.
    pub history: History,
    /// The credential to fetch with, when the repository is private.
    pub grant: Option<Grant>,
}

impl Want {
    /// The repository at `locator`, at its `HEAD`, with its history, and no credential.
    pub fn new(locator: Locator) -> Self {
        Self { locator, revision: None, history: History::Wanted, grant: None }
    }
}

/// What the host holds after a fetch: a handle to the repository it cached, and the commit the
/// request's revision named in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cloned {
    /// The repository's place in the host's cache: the [`Request::key`].
    pub clone: Hash,
    /// The commit the revision names, which a snapshot of the clone is taken at.
    pub commit: Oid,
    /// Whether the history is cut short of the root commit.
    pub shallow: bool,
    /// How many objects the clone holds.
    pub objects: u64,
    /// How large the clone is on the host's disk.
    pub bytes: Bytes,
}

/// What the builder lets a plugin fetch (an operator's limit, decision 0044).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Network {
    /// No repository is fetched, not even one on the builder's own disk.
    #[default]
    Off,
    /// Only repositories on the builder's own disk, `file://`.
    Local,
    /// Every scheme the builder has a transport for.
    On,
}

impl Network {
    /// How the setting is named.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Local => "local",
            Self::On => "on",
        }
    }

    /// Whether a repository at `scheme` may be fetched.
    pub const fn allows(self, scheme: Scheme) -> bool {
        matches!((self, scheme), (Self::On, _) | (Self::Local, Scheme::File))
    }
}

impl fmt::Display for Network {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// A budget a fetch passed, with the number and the limit (architecture.md section 13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Budget {
    /// The clone is larger than a source may be: `source.budget.bytes`.
    Bytes {
        /// How large it was when the fetch stopped.
        size: Bytes,
        /// The most it may be.
        limit: Bytes,
    },
    /// The clone holds more objects than a source may: `source.budget.objects`.
    Objects {
        /// How many it held.
        count: u64,
        /// The most it may hold.
        limit: u64,
    },
}

/// Why a credential was not used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Auth {
    /// The request names a credential the host does not hold.
    Absent,
}

/// What the host refused of a fetch or a read. `E` is the cause of a failure with no case of its
/// own: in the host, the error itself; in a plugin, a marker, since the host has already said
/// why.
#[derive(Debug)]
#[non_exhaustive]
pub enum Refused<E> {
    /// The builder's network setting forbids the scheme.
    Network {
        /// The scheme asked for.
        scheme: Scheme,
        /// The setting that forbids it.
        policy: Network,
    },
    /// The credential asked for was not used.
    Auth {
        /// The credential's name.
        grant: Grant,
        /// Why not.
        outcome: Auth,
    },
    /// A budget was passed, and the clone was removed.
    Budget(Budget),
    /// The revision names nothing the clone holds.
    Revision {
        /// The revision, as asked.
        written: Revision,
    },
    /// The builder has no transport for the scheme.
    Transport {
        /// The scheme.
        scheme: Scheme,
    },
    /// Anything else: an unreachable remote, a damaged clone, a disk that refused.
    Failed(E),
}

impl<E: fmt::Display> fmt::Display for Refused<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network { scheme, policy } => {
                write!(out, "{scheme} repositories are not fetched: network is {policy}")
            }
            Self::Auth { grant, .. } => write!(out, "the credential `{grant}` is not held"),
            Self::Budget(Budget::Bytes { size, limit }) => {
                write!(out, "the clone is {size}; a source may hold {limit}")
            }
            Self::Budget(Budget::Objects { count, limit }) => {
                write!(out, "the clone holds {count} objects; a source may hold {limit}")
            }
            Self::Revision { written } => write!(out, "`{written}` names nothing the clone holds"),
            Self::Transport { scheme } => write!(out, "there is no transport for {scheme} repositories"),
            Self::Failed(error) => write!(out, "{error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Refused<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Failed(error) => Some(error),
            _ => None,
        }
    }
}

impl<E: std::error::Error> From<&Refused<E>> for Diagnostic {
    /// The refusal as the author or operator sees it. A failure's causes are written out once
    /// here, at the edge, as the catalog's `reason`.
    fn from(refused: &Refused<E>) -> Self {
        Self::new(match refused {
            Refused::Network { scheme, policy } => {
                Problem::SourceGitNetwork { scheme: scheme.to_string(), policy: policy.to_string() }
            }
            Refused::Auth { grant, .. } => Problem::SourceGitAuth { grant: grant.to_string() },
            Refused::Budget(Budget::Bytes { size, limit }) => Problem::SourceBudgetBytes { size: *size, limit: *limit },
            Refused::Budget(Budget::Objects { count, limit }) => {
                Problem::SourceBudgetObjects { count: *count, limit: *limit }
            }
            Refused::Revision { written } => Problem::SourceGitRevision { written: written.to_string() },
            Refused::Transport { scheme } => Problem::SourceGitTransport { scheme: scheme.to_string() },
            Refused::Failed(error) => Problem::SourceRepositoryUnreadable { reason: Chain(error).to_string() },
        })
    }
}

/// An error and its causes as one line, for a diagnostic's `reason`.
struct Chain<'e, E>(&'e E);

impl<E: std::error::Error> fmt::Display for Chain<'_, E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)?;
        let mut cause = self.0.source();
        while let Some(error) = cause {
            write!(out, ": {error}")?;
            cause = error.source();
        }
        Ok(())
    }
}

/// The host's `git` capability, as a trait a source's rule is written against.
pub trait Git {
    /// What a failure with no case of its own holds.
    type Error: std::error::Error + 'static;

    /// Fetches the repository into the host's cache, or brings a clone already there to the
    /// depth asked for.
    fn fetch(&mut self, request: &Request) -> Result<Cloned, Refused<Self::Error>>;

    /// The snapshot of `cloned` at its commit, its files stored where the build reads them.
    fn snapshot(&mut self, cloned: &Cloned) -> Result<Snapshot, Refused<Self::Error>>;
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn the_cache_key_names_the_grant() {
        let locator: Locator = "https://example.org/a.git".parse().unwrap();
        let open = Request::new(locator.clone());
        let granted = Request { grant: Some("deploy".parse().unwrap()), ..Request::new(locator) };
        assert_ne!(open.key(), granted.key());
    }

    #[test]
    fn a_locator_is_a_file_or_https_address_and_nothing_else() {
        assert_eq!("file:///srv/repo".parse::<Locator>().unwrap().scheme(), Scheme::File);
        assert_eq!(
            "https://example.org/a/b.git".parse::<Locator>().unwrap().to_string(),
            "https://example.org/a/b.git"
        );
        for refused in [
            "git@host:a/b.git",
            "ssh://host/a",
            "git://host/a",
            "http://host/a",
            "file://relative",
            "https://",
            "file:///a b",
        ] {
            assert!(refused.parse::<Locator>().is_err(), "{refused}");
        }
        assert_eq!("git@host:a".parse::<Locator>(), Err(LocatorError::Scheme { written: "git@host:a".into() }));
    }

    #[test]
    fn a_revision_cannot_look_like_an_option() {
        assert!("main".parse::<Revision>().is_ok());
        assert_eq!("--upload-pack=x".parse::<Revision>(), Err(NameError::Character { at: 0 }));
        assert!("a b".parse::<Revision>().is_err());
        assert!("".parse::<Revision>().is_err());
    }

    #[test]
    fn a_grant_is_a_plain_name() {
        assert!("github".parse::<Grant>().is_ok());
        assert!("GitHub".parse::<Grant>().is_err());
        assert!("a".repeat(65).parse::<Grant>().is_err());
    }

    #[test]
    fn the_network_setting_orders_what_it_lets_through() {
        assert!(!Network::Off.allows(Scheme::File));
        assert!(Network::Local.allows(Scheme::File) && !Network::Local.allows(Scheme::Https));
        assert!(Network::On.allows(Scheme::Https));
    }
}
