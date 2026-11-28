//! What the R2 adapter fails with.

use std::fmt;

use model::Hash;

/// What the adapter asked R2 for when it refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Call {
    /// The whole object in one request.
    Put,
    /// The start of a multipart upload.
    Begin,
    /// One part of it, counting from one.
    Part(u16),
    /// The end of it.
    Complete,
    /// Giving it up.
    Abort,
    /// The metadata of an object.
    Head,
    /// An object with its body.
    Get,
    /// A page of keys.
    List,
    /// Removing an object.
    Delete,
    /// Reading the next chunk of a body.
    Chunk,
}

impl fmt::Display for Call {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Put => out.write_str("a put"),
            Self::Begin => out.write_str("the start of a multipart upload"),
            Self::Part(number) => write!(out, "part {number} of a multipart upload"),
            Self::Complete => out.write_str("the end of a multipart upload"),
            Self::Abort => out.write_str("an abort of a multipart upload"),
            Self::Head => out.write_str("a head"),
            Self::Get => out.write_str("a get"),
            Self::List => out.write_str("a listing"),
            Self::Delete => out.write_str("a delete"),
            Self::Chunk => out.write_str("a read of the body"),
        }
    }
}

/// Why a write was given up.
#[derive(Debug)]
#[non_exhaustive]
pub enum Reason {
    /// The bytes did not hash to the name.
    Mismatch {
        /// The hash named.
        expected: Hash,
        /// The hash of the bytes.
        found: Hash,
    },
    /// A call failed.
    Failed(Box<Error>),
}

/// A failure of the R2 adapter.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// R2 refused a call, or the JavaScript boundary did.
    Call {
        /// What was asked.
        call: Call,
        /// The key it concerned.
        key: String,
        /// What R2 said.
        source: worker::Error,
    },
    /// An object of a tenant's prefix is not named by a hash.
    Stray {
        /// The key.
        key: String,
    },
    /// An object has no body although it was just found: it was removed between the two calls.
    Bodiless {
        /// The key.
        key: String,
    },
    /// A write failed after parts were uploaded, and giving the upload up failed too, so R2 keeps
    /// the parts until its own expiry.
    Abandoned {
        /// The key.
        key: String,
        /// Why the abort failed.
        source: worker::Error,
        /// Why the write was being given up.
        reason: Reason,
    },
    /// R2 said a listing goes on and gave no cursor to continue it.
    Unfinished {
        /// The prefix listed.
        prefix: String,
    },
    /// A blob needs more parts than R2 takes.
    Parts {
        /// The key.
        key: String,
        /// The parts R2 takes.
        most: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Call { call, key, .. } => write!(out, "R2 refused {call} for {key}"),
            Self::Stray { key } => write!(out, "{key} is in the bucket and is not named by a hash"),
            Self::Bodiless { key } => write!(out, "{key} was found and has no body"),
            Self::Abandoned { key, .. } => write!(out, "a failed write of {key} left its parts in a multipart upload"),
            Self::Unfinished { prefix } => write!(out, "R2 said the listing of {prefix} goes on and gave no cursor"),
            Self::Parts { key, most } => write!(out, "{key} needs more than the {most} parts R2 takes"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Call { source, .. } | Self::Abandoned { source, .. } => Some(source),
            Self::Stray { .. } | Self::Bodiless { .. } | Self::Unfinished { .. } | Self::Parts { .. } => None,
        }
    }
}

impl From<Error> for blob::Error<Error> {
    fn from(source: Error) -> Self {
        Self::Backend { source }
    }
}

/// A key as the adapter writes it: the tenant, a slash and the hash.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Key<'a> {
    pub(crate) tenant: &'a blob::Tenant,
    pub(crate) hash: Hash,
}

impl fmt::Display for Key<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}/{}", self.tenant, self.hash)
    }
}
