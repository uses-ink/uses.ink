//! Where a repository is, and the names that pick what to fetch of it.

use std::fmt;
use std::str::FromStr;

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
            return Err(LocatorError::Character { at: written.len() + "://".len() + at });
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

impl serde::Serialize for Locator {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
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
