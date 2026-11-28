//! Why bytes are no archive of files.

use std::fmt;

use model::PathError;

/// An archive that cannot be taken, and the fact that makes it wrong.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The archive ends inside a header or a file, at this offset.
    Truncated {
        /// Where.
        at: usize,
    },
    /// A header whose checksum does not hold.
    Checksum {
        /// Where the header is.
        at: usize,
    },
    /// A number field that is not octal.
    Number {
        /// Where the header is.
        at: usize,
    },
    /// An entry that is not a regular file or a folder: a link, a device, a pax header.
    Kind {
        /// Where the header is.
        at: usize,
        /// Its type flag.
        flag: u8,
    },
    /// A name that is not text, or not a path of a folder.
    Name {
        /// Where the header is.
        at: usize,
        /// Why, when it is text.
        source: Option<PathError>,
    },
    /// Two entries with one name.
    Duplicate {
        /// The name.
        name: String,
    },
    /// More files than the limit.
    Files {
        /// The limit.
        limit: u64,
    },
    /// A file larger than the limit.
    File {
        /// The name.
        name: String,
        /// How large it says it is.
        size: u64,
        /// The limit.
        limit: u64,
    },
    /// All files together larger than the limit.
    Total {
        /// The size so far, with the file that went over.
        size: u64,
        /// The limit.
        limit: u64,
    },
    /// A name longer than the limit.
    Long {
        /// How long.
        size: usize,
        /// The limit.
        limit: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { at } => write!(out, "the archive ends early, at byte {at}"),
            Self::Checksum { at } => write!(out, "the header at byte {at} has a checksum that does not hold"),
            Self::Number { at } => write!(out, "the header at byte {at} has a number that is not octal"),
            Self::Kind { at, flag } => {
                write!(out, "the entry at byte {at} has type {flag:#04x}: only files and folders are taken")
            }
            Self::Name { at, .. } => write!(out, "the entry at byte {at} has a name that is no path in a folder"),
            Self::Duplicate { name } => write!(out, "the archive holds {name} twice"),
            Self::Files { limit } => write!(out, "the archive holds more than {limit} files"),
            Self::File { name, size, limit } => {
                write!(out, "{name} is {size} bytes, over the limit of {limit} a file")
            }
            Self::Total { size, limit } => write!(out, "the files hold {size} bytes, over the limit of {limit}"),
            Self::Long { size, limit } => write!(out, "a name of {size} bytes is over the limit of {limit}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Name { source: Some(source), .. } => Some(source),
            _ => None,
        }
    }
}
