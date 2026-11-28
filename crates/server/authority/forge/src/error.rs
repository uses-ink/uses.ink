//! What a delivery or a call to a forge fails with.

use std::fmt;

use crate::{CommitError, Delivery, NameError};

/// A delivery that was refused. A refusal says what the check was and nothing of why it failed:
/// a wrong secret, a tampered body and a malformed signature are all [`Refused::Signature`].
#[derive(Debug)]
#[non_exhaustive]
pub enum Refused {
    /// The body is larger than the setting allows.
    Oversize {
        /// How many bytes it has.
        size: usize,
        /// The most that is read.
        limit: usize,
    },
    /// A header the forge always sends is not there.
    Missing {
        /// Its name.
        header: &'static str,
    },
    /// The signature or token does not authenticate the body.
    Signature,
    /// The delivery id was seen inside the window.
    Replayed {
        /// The id.
        delivery: Delivery,
    },
    /// The delivery id is no id.
    Delivery(NameError),
    /// The signed body is not the JSON of the event.
    Json(serde_json::Error),
    /// The repository the event names is no repository.
    Name(NameError),
    /// The commit the event names is no commit id.
    Commit(CommitError),
}

impl Refused {
    /// The code of the catalog (`crates/core/diag/catalog.toml`) that words this.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Oversize { .. } => "hook.body.large",
            Self::Missing { .. } => "hook.header.missing",
            Self::Signature => "hook.signature.invalid",
            Self::Replayed { .. } => "hook.delivery.replayed",
            Self::Delivery(_) | Self::Json(_) | Self::Name(_) | Self::Commit(_) => "hook.payload.malformed",
        }
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversize { size, limit } => {
                write!(out, "the delivery body is {size} bytes and only {limit} are read")
            }
            Self::Missing { header } => write!(out, "the delivery has no `{header}` header"),
            Self::Signature => out.write_str("the delivery's signature does not authenticate its body"),
            Self::Replayed { delivery } => write!(out, "the delivery {delivery} was seen already"),
            Self::Delivery(_) => out.write_str("the delivery id is not an id"),
            Self::Json(_) => out.write_str("the delivery body is not the JSON of an event"),
            Self::Name(_) => out.write_str("the event names a repository that is not a repository path"),
            Self::Commit(_) => out.write_str("the event names a commit that is not a commit id"),
        }
    }
}

impl std::error::Error for Refused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Delivery(source) | Self::Name(source) => Some(source),
            Self::Json(source) => Some(source),
            Self::Commit(source) => Some(source),
            Self::Oversize { .. } | Self::Missing { .. } | Self::Signature | Self::Replayed { .. } => None,
        }
    }
}

impl From<crate::Replayed> for Refused {
    fn from(replayed: crate::Replayed) -> Self {
        Self::Replayed { delivery: replayed.delivery }
    }
}

/// A call to a forge's API that failed. `E` is the transport's own error.
#[derive(Debug)]
#[non_exhaustive]
pub enum Fail<E> {
    /// The URL could not be built.
    Url(url::ParseError),
    /// The transport got no answer.
    Transport(E),
    /// The reply is larger than the setting allows.
    Oversize {
        /// How many bytes it has.
        size: usize,
        /// The most that is read.
        limit: usize,
    },
    /// The forge answered with this status where it should not have.
    Status {
        /// The status code.
        status: u16,
    },
    /// The reply is not the JSON that was expected.
    Json(serde_json::Error),
}

impl<E> Fail<E> {
    /// The code of the catalog that words this.
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Url(_) | Self::Transport(_) => "forge.call.failed",
            Self::Oversize { .. } => "forge.reply.large",
            Self::Status { .. } => "forge.call.refused",
            Self::Json(_) => "forge.reply.malformed",
        }
    }
}

impl<E> fmt::Display for Fail<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Url(_) => out.write_str("the forge's URL could not be built"),
            Self::Transport(_) => out.write_str("the forge did not answer"),
            Self::Oversize { size, limit } => {
                write!(out, "the forge's reply is {size} bytes and only {limit} are read")
            }
            Self::Status { status } => write!(out, "the forge answered with status {status}"),
            Self::Json(_) => out.write_str("the forge's reply is not the JSON that was expected"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for Fail<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Url(source) => Some(source),
            Self::Transport(source) => Some(source),
            Self::Json(source) => Some(source),
            Self::Oversize { .. } | Self::Status { .. } => None,
        }
    }
}
