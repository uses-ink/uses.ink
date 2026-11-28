//! What a lookup through a real DNS server fails with, each with its cause.

use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use super::wire::Malformed;
use crate::NoResolver;

/// Why no nameserver could be found to ask.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unlisted {
    /// The file of nameservers could not be read.
    Read {
        /// The file.
        path: PathBuf,
        /// What the system said.
        source: std::io::Error,
    },
    /// The file names no nameserver this can ask.
    None {
        /// The file.
        path: PathBuf,
        /// How many `nameserver` lines it has that are not an address this speaks to (an
        /// address with a scope, say).
        skipped: usize,
    },
}

impl fmt::Display for Unlisted {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, .. } => {
                write!(out, "{} could not be read, so there is no nameserver to ask", path.display())
            }
            Self::None { path, skipped } => {
                write!(out, "{} names no nameserver to ask ({skipped} lines were not an address)", path.display())
            }
        }
    }
}

impl std::error::Error for Unlisted {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read { source, .. } => Some(source),
            Self::None { .. } => None,
        }
    }
}

/// A lookup that got no answer from the resolver it was given.
#[derive(Debug)]
#[non_exhaustive]
pub enum Failed {
    /// The server has no resolver: the operator chose none.
    Offline(NoResolver),
    /// There is no nameserver to ask.
    Servers(Unlisted),
    /// No random bytes were available for the question's id.
    Draw(Box<dyn std::error::Error>),
    /// The question could not be written.
    Question(Malformed),
    /// The connection to the nameserver failed.
    Io {
        /// The nameserver.
        server: SocketAddr,
        /// What the system said.
        source: std::io::Error,
    },
    /// The nameserver answered with a message that is not a DNS answer to the question.
    Malformed {
        /// The nameserver.
        server: SocketAddr,
        /// Why.
        source: Malformed,
    },
    /// The nameserver answered that it cannot or will not answer.
    Rcode {
        /// The nameserver.
        server: SocketAddr,
        /// Its response code: 2 is a server failure, 5 a refusal.
        code: u8,
    },
    /// The nameserver did not answer in the time it was given.
    Timeout {
        /// The nameserver.
        server: SocketAddr,
        /// How long it was given.
        after: Duration,
    },
    /// Two resolvers that must agree gave different records.
    Disagree {
        /// How many records the first gave.
        first: usize,
        /// How many records the second gave.
        second: usize,
    },
    /// Every nameserver failed; the last one's failure is the cause.
    Exhausted {
        /// How many were asked.
        tried: usize,
        /// The last failure.
        last: Box<Failed>,
    },
}

impl fmt::Display for Failed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Offline(_) => out.write_str("this server has no DNS resolver"),
            Self::Servers(_) => out.write_str("there is no nameserver to ask"),
            Self::Draw(_) => out.write_str("no random bytes were available for the question's id"),
            Self::Question(_) => out.write_str("the question could not be written"),
            Self::Io { server, .. } => write!(out, "the nameserver {server} could not be reached"),
            Self::Malformed { server, .. } => write!(out, "the nameserver {server} sent a message that is no answer"),
            Self::Rcode { server, code } => {
                let name = match code {
                    2 => "a server failure",
                    5 => "a refusal",
                    _ => "an error",
                };
                write!(out, "the nameserver {server} answered with {name} (code {code})")
            }
            Self::Timeout { server, after } => {
                write!(out, "the nameserver {server} did not answer in {} ms", after.as_millis())
            }
            Self::Disagree { first, second } => {
                write!(out, "the two resolvers disagree: the first gave {first} records, the second {second}")
            }
            Self::Exhausted { tried, .. } => write!(out, "all {tried} nameservers failed"),
        }
    }
}

impl std::error::Error for Failed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Offline(source) => Some(source),
            Self::Servers(source) => Some(source),
            Self::Draw(source) => Some(source.as_ref()),
            Self::Question(source) | Self::Malformed { source, .. } => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::Exhausted { last, .. } => Some(last.as_ref()),
            Self::Rcode { .. } | Self::Timeout { .. } | Self::Disagree { .. } => None,
        }
    }
}
