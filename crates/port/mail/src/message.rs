//! Messages and the templates they are written from.

use std::fmt;
use std::time::Duration;

use crate::{Address, Link};

/// What a message says. Each variant knows its subject and, through `Display`, its body, so no
/// call site builds text.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Template {
    /// A one-time link that signs the reader in.
    Login {
        /// The link.
        link: Link,
        /// How long it works.
        valid: Duration,
    },
    /// A one-time link that proves the reader can read mail at this address and signs nobody in.
    Prove {
        /// The link.
        link: Link,
        /// How long it works.
        valid: Duration,
    },
}

impl Template {
    /// The subject line.
    pub const fn subject(&self) -> &'static str {
        match self {
            Self::Login { .. } => "Your sign-in link",
            Self::Prove { .. } => "Confirm this address",
        }
    }
}

impl fmt::Display for Template {
    /// The body, as plain text.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Login { link, valid } => write!(
                out,
                "Open this link to sign in:\n\n{link}\n\nIt works once, for {} minutes. If you did not ask for it, ignore this message.\n",
                valid.as_secs().div_ceil(60)
            ),
            Self::Prove { link, valid } => write!(
                out,
                "Open this link to confirm that you read mail at this address:\n\n{link}\n\nIt works once, for {} minutes, and it does not sign you in. If you did not ask for it, ignore this message.\n",
                valid.as_secs().div_ceil(60)
            ),
        }
    }
}

/// One message to one person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// Who gets it.
    pub to: Address,
    /// What it says.
    pub template: Template,
}
