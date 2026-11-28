//! The repo challenge: a file in the repository that carries a token we issued.

use std::fmt;

use crate::{Fail, Host, Repo, Secret, Segment};

/// A token a person commits to `.well-known/uses-ink` of the default branch to show they can write
/// to the repository. The caller draws its 16 bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Challenge([u8; 16]);

impl Challenge {
    /// What a line starts with.
    pub const PREFIX: &'static str = "uses-ink-repo=";
    /// The file it is committed to.
    pub const FILE: &'static str = ".well-known/uses-ink";

    /// The challenge of `bytes`.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Whether a line of `text` is this challenge. Lines are trimmed; the compare is constant-time.
    pub fn found_in(&self, text: &str) -> bool {
        let ours = Secret::new(self.to_string());
        text.lines().any(|line| ours.matches(line.trim().as_bytes()))
    }
}

impl fmt::Display for Challenge {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(Self::PREFIX)?;
        self.0.iter().try_for_each(|byte| write!(out, "{byte:02x}"))
    }
}

impl fmt::Debug for Challenge {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("Challenge(<redacted>)")
    }
}

/// That whoever holds the challenge can write to the repository, and the namespace it is under.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Proven {
    /// The repository.
    pub repo: Repo,
    /// Its top-level user, organization or group.
    pub owner: Segment,
}

impl Proven {
    /// The proof that `repo` carries the challenge.
    pub fn of(repo: Repo) -> Self {
        let owner = repo.path.owner().clone();
        Self { repo, owner }
    }
}

/// A proof that failed or was refused. `E` is the transport's own error.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unproven<E> {
    /// A call to the forge failed.
    Call(Fail<E>),
    /// The forge is not one this authority talks to. An instance is added by its operator; a call
    /// to a host a stranger names would be a request the server sends for them.
    Unlisted {
        /// The host.
        host: Host,
    },
    /// The repository has no challenge file.
    Absent,
    /// The challenge file does not carry this challenge.
    Wrong,
}

impl<E> Unproven<E> {
    /// The code of the catalog that words this.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Call(fail) => fail.code(),
            Self::Unlisted { .. } => "repo.host.unlisted",
            Self::Absent => "repo.proof.missing",
            Self::Wrong => "repo.proof.wrong",
        }
    }
}

impl<E> fmt::Display for Unproven<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Call(_) => out.write_str("the forge could not be asked for the challenge file"),
            Self::Unlisted { host } => write!(out, "{host} is not a forge this server talks to"),
            Self::Absent => write!(out, "the repository has no `{}`", Challenge::FILE),
            Self::Wrong => write!(out, "`{}` does not carry the challenge", Challenge::FILE),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Unproven<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Call(source) => Some(source),
            Self::Unlisted { .. } | Self::Absent | Self::Wrong => None,
        }
    }
}

impl<E> From<Fail<E>> for Unproven<E> {
    fn from(source: Fail<E>) -> Self {
        Self::Call(source)
    }
}

impl Challenge {
    /// Reads what the challenge file answered: `text` is its body, `None` when the forge has no
    /// such file. The one place both forges turn a fetched file into a proof.
    pub fn check<E>(&self, repo: Repo, text: Option<&str>) -> Result<Proven, Unproven<E>> {
        match text {
            None => Err(Unproven::Absent),
            Some(text) if self.found_in(text) => Ok(Proven::of(repo)),
            Some(_) => Err(Unproven::Wrong),
        }
    }
}
