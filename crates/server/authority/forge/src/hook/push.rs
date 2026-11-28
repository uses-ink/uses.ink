//! A push, as the forges report it, and what is not one.

use std::collections::BTreeSet;
use std::fmt;

use build::{Branch, Revision, RevisionError};

use crate::{NameError, Refused, Repo};

/// Text that is no commit id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommitError {
    /// Not 40 or 64 characters.
    Length {
        /// How long.
        found: usize,
    },
    /// Not a lower-case hex digit, at this offset.
    Digit {
        /// The offset.
        at: usize,
    },
}

impl fmt::Display for CommitError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => write!(out, "a commit id is 40 or 64 hex digits, not {found}"),
            Self::Digit { at } => write!(out, "byte {at} of the commit id is not a lower-case hex digit"),
        }
    }
}

impl std::error::Error for CommitError {}

/// The repositories that sites follow. An event for any other is ignored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Roster(BTreeSet<Repo>);

impl Roster {
    /// A roster that follows nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// Follows `repo` too.
    pub fn follow(&mut self, repo: Repo) {
        self.0.insert(repo);
    }

    /// Whether a site follows `repo`.
    pub fn follows(&self, repo: &Repo) -> bool {
        self.0.contains(repo)
    }
}

impl FromIterator<Repo> for Roster {
    fn from_iter<I: IntoIterator<Item = Repo>>(repos: I) -> Self {
        Self(repos.into_iter().collect())
    }
}

/// A branch of a followed repository moved to a commit.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Push {
    /// The repository.
    pub repo: Repo,
    /// The branch.
    pub branch: Branch,
    /// The commit it is at now.
    pub revision: Revision,
    /// The repository's default branch, as the event says it, or none when it says none or one
    /// this does not read. A site that follows no branch by name follows this one.
    pub default: Option<Branch>,
}

/// A commit of a repository: where a status is posted.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Pinned {
    /// The repository.
    pub repo: Repo,
    /// The commit.
    pub revision: Revision,
}

impl Pinned {
    /// `revision` of `repo`.
    pub const fn new(repo: Repo, revision: Revision) -> Self {
        Self { repo, revision }
    }
}

/// Why a verified delivery queues nothing. It is a fact the caller reports, not an error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Ignored {
    /// The event is not a push (`ping`, `issues`, a tag push hook).
    Event {
        /// The event as the forge named it.
        written: String,
    },
    /// No site follows the repository.
    Unfollowed {
        /// The repository.
        repo: Repo,
    },
    /// The push is to a tag or another reference, not a branch.
    Ref {
        /// The reference as written.
        written: String,
    },
    /// The branch's name is none this reads.
    Branch {
        /// The name as written.
        written: String,
        /// Why it is no branch.
        source: RevisionError,
    },
    /// The push deleted the branch.
    Deleted {
        /// The branch.
        branch: Branch,
    },
}

impl Ignored {
    /// The code of the catalog that words this.
    pub const fn code(&self) -> &'static str {
        "hook.event.ignored"
    }
}

impl fmt::Display for Ignored {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Event { written } => write!(out, "`{written}` is not a push"),
            Self::Unfollowed { repo } => write!(out, "no site follows {repo}"),
            Self::Ref { written } => write!(out, "`{written}` is not a branch"),
            Self::Branch { written, source } => write!(out, "`{written}` is not a branch this reads: {source}"),
            Self::Deleted { branch } => write!(out, "the push deleted the branch `{}`", branch.as_str()),
        }
    }
}

/// What a verified delivery came to.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Received {
    /// A push to a followed branch: queue a build.
    Push(Push),
    /// Nothing to queue, and why.
    Ignored(Ignored),
}

impl Push {
    /// The prefix of a branch's reference.
    pub const HEADS: &'static str = "refs/heads/";

    /// What a push to `repo` that moved `reference` to `after` comes to, for a `roster`. A commit
    /// of zeros is the forge's way to say the reference was deleted. `tip` is the reference, the
    /// commit it moved to and the repository's default branch as the event writes it.
    pub fn read(repo: Repo, tip: (&str, &str, Option<&str>), roster: &Roster) -> Result<Received, Refused> {
        let (reference, after, default) = tip;
        if !roster.follows(&repo) {
            return Ok(Received::Ignored(Ignored::Unfollowed { repo }));
        }
        let Some(written) = reference.strip_prefix(Self::HEADS) else {
            return Ok(Received::Ignored(Ignored::Ref { written: reference.to_owned() }));
        };
        let branch = match written.parse::<Branch>() {
            Ok(branch) => branch,
            Err(source) => return Ok(Received::Ignored(Ignored::Branch { written: written.to_owned(), source })),
        };
        if !after.is_empty() && after.bytes().all(|byte| byte == b'0') {
            return Ok(Received::Ignored(Ignored::Deleted { branch }));
        }
        let revision = Commit::read(after).map_err(Refused::Commit)?;
        let default = default.and_then(|written| written.parse::<Branch>().ok());
        Ok(Received::Push(Self { repo, branch, revision, default }))
    }

    /// The host of the forge that sent an event, from the `https` address of the repository it
    /// names. A forge that names none, or one that is no DNS name, is a malformed event.
    pub fn host(address: &str) -> Result<crate::Host, Refused> {
        let url = url::Url::parse(address).map_err(|_| Refused::Name(NameError::Empty))?;
        url.host_str().ok_or(Refused::Name(NameError::Empty))?.parse().map_err(Refused::Name)
    }

    /// The repository a forge's `full_name` names, on `host`.
    pub fn repo(host: crate::Host, full: &str) -> Result<Repo, Refused> {
        let path = full.parse().map_err(|source: NameError| Refused::Name(source))?;
        Ok(Repo::new(host, path))
    }
}

/// A commit id as a revision.
#[derive(Clone, Copy, Debug)]
pub struct Commit;

impl Commit {
    /// The revision `text` names: 40 or 64 lower-case hex digits.
    pub fn read(text: &str) -> Result<Revision, CommitError> {
        if text.len() != 40 && text.len() != 64 {
            return Err(CommitError::Length { found: text.len() });
        }
        if let Some(at) = text.bytes().position(|byte| !matches!(byte, b'0'..=b'9' | b'a'..=b'f')) {
            return Err(CommitError::Digit { at });
        }
        // Hex digits are none of the characters a revision refuses, so this holds; if a rule
        // of `Revision` ever changes it fails here and not later.
        text.parse().map_err(|_| CommitError::Digit { at: 0 })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Host;

    fn repo() -> Repo {
        Repo::new(Host::GITHUB, "octo/docs".parse().unwrap())
    }

    const SHA: &str = "6dcb09b5b57875f334f61aebed695e2e4193db5e";

    #[test]
    fn a_push_to_a_followed_branch_is_a_push() {
        let roster: Roster = [repo()].into_iter().collect();
        let got = Push::read(repo(), ("refs/heads/main", SHA, None), &roster).unwrap();
        let Received::Push(push) = got else { panic!("{got:?}") };
        assert_eq!((push.branch.as_str(), push.revision.as_str()), ("main", SHA));
    }

    #[test]
    fn a_push_carries_the_default_branch_the_event_names() {
        let roster: Roster = [repo()].into_iter().collect();
        let read = |default| match Push::read(repo(), ("refs/heads/dev", SHA, default), &roster).unwrap() {
            Received::Push(push) => push.default.map(|branch| branch.as_str().to_owned()),
            other => panic!("{other:?}"),
        };
        assert_eq!(read(Some("main")).as_deref(), Some("main"));
        assert_eq!(read(None), None);
        assert_eq!(read(Some("-bad")), None, "a default this does not read is none, not a refusal of the push");
    }

    #[test]
    fn the_host_of_an_event_is_read_from_its_address() {
        assert_eq!(Push::host("https://Codeberg.org/octo/docs").unwrap().as_str(), "codeberg.org");
        assert!(Push::host("not a url").is_err());
        assert!(Push::host("").is_err());
    }

    #[test]
    fn everything_else_is_ignored_with_a_reason() {
        let roster: Roster = [repo()].into_iter().collect();
        let zeros = "0".repeat(40);
        let other = Repo::new(Host::GITHUB, "x/y".parse().unwrap());
        assert!(matches!(
            Push::read(other, ("refs/heads/a", SHA, None), &roster),
            Ok(Received::Ignored(Ignored::Unfollowed { .. }))
        ));
        assert!(matches!(
            Push::read(repo(), ("refs/tags/v1", SHA, None), &roster),
            Ok(Received::Ignored(Ignored::Ref { .. }))
        ));
        assert!(matches!(
            Push::read(repo(), ("refs/heads/a", &zeros, None), &roster),
            Ok(Received::Ignored(Ignored::Deleted { .. }))
        ));
        assert!(matches!(
            Push::read(repo(), ("refs/heads/-x", SHA, None), &roster),
            Ok(Received::Ignored(Ignored::Branch { .. }))
        ));
        assert!(matches!(
            Push::read(repo(), ("refs/heads/a", "abc", None), &roster),
            Err(Refused::Commit(CommitError::Length { found: 3 }))
        ));
    }
}
