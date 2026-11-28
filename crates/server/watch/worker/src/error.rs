//! Why a head could not be read.

use std::fmt;

use model::git::{Network, Scheme};
use watch::HeadError;

/// The advertisement is not one this reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Misread {
    /// A packet length that is not four hexadecimal digits or is under four, at this byte offset.
    Length {
        /// Where.
        at: usize,
    },
    /// The body ends inside a packet, at this byte offset.
    Truncated {
        /// Where.
        at: usize,
    },
    /// It does not open with the `# service=git-upload-pack` packet and a flush: not a smart server.
    Service,
    /// A reference line that is not `<id> <name>`, at this byte offset.
    Line {
        /// Where.
        at: usize,
    },
    /// The server speaks protocol version 2, which a request without `Git-Protocol` does not get.
    Version,
}

impl fmt::Display for Misread {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { at } => write!(out, "the packet at byte {at} has no valid length"),
            Self::Truncated { at } => write!(out, "the body ends inside the packet at byte {at}"),
            Self::Service => out.write_str("the answer is not a git smart-HTTP advertisement"),
            Self::Line { at } => write!(out, "the reference at byte {at} is not `<id> <name>`"),
            Self::Version => out.write_str("the server answered with protocol version 2"),
        }
    }
}

impl std::error::Error for Misread {}

/// The remote has nothing for the revision asked.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Absent {
    /// No revision was asked and the remote's `HEAD` has no commit yet.
    Unborn,
    /// No revision was asked and the remote advertises no `HEAD`.
    NoHead,
    /// No branch, tag or commit of this name.
    Revision {
        /// The revision as written.
        written: String,
    },
}

impl fmt::Display for Absent {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unborn => out.write_str("the remote's default branch has no commit yet"),
            Self::NoHead => out.write_str("the remote advertises no default branch"),
            Self::Revision { written } => write!(out, "the remote has no branch, tag or commit `{written}`"),
        }
    }
}

impl std::error::Error for Absent {}

/// A remote that gave no head. `E` is the fetch's own error.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unlisted<E> {
    /// The operator's network setting does not allow the repository's scheme.
    Network {
        /// The scheme.
        scheme: Scheme,
        /// The setting.
        policy: Network,
    },
    /// A Worker reaches repositories over https only.
    Scheme {
        /// The scheme.
        scheme: Scheme,
    },
    /// The call got no answer.
    Fetch(E),
    /// The server answered with a status that is no advertisement (a private repository asks for
    /// a login with 401, a missing one is 404).
    Status {
        /// The code.
        code: u16,
    },
    /// The answer is over the limit.
    Large {
        /// The limit in bytes.
        most: usize,
    },
    /// The answer is no advertisement this reads.
    Advert(Misread),
    /// The remote has no commit for the revision.
    Absent(Absent),
    /// The commit the remote named is no commit id.
    Commit(HeadError),
}

impl<E> fmt::Display for Unlisted<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network { scheme, policy } => write!(out, "the network setting `{policy}` does not allow {scheme}"),
            Self::Scheme { scheme } => write!(out, "a Worker reads repositories over https, not {scheme}"),
            Self::Fetch(_) => out.write_str("the remote did not answer"),
            Self::Status { code } => write!(out, "the remote answered {code} to its reference list"),
            Self::Large { most } => write!(out, "the remote's reference list is over {most} bytes"),
            Self::Advert(_) => out.write_str("the remote's reference list could not be read"),
            Self::Absent(_) => out.write_str("the remote has no such commit"),
            Self::Commit(_) => out.write_str("the remote named a commit that is no commit id"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Unlisted<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Fetch(source) => Some(source),
            Self::Advert(source) => Some(source),
            Self::Absent(source) => Some(source),
            Self::Commit(source) => Some(source),
            Self::Network { .. } | Self::Scheme { .. } | Self::Status { .. } | Self::Large { .. } => None,
        }
    }
}
