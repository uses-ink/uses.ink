//! What goes in and what comes out.

use std::fmt;
use std::str::FromStr;

/// A message's identity in its queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MessageId(pub u64);

impl fmt::Display for MessageId {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)
    }
}

/// A key that makes a message once: while a message with this key is in the queue (waiting or
/// leased), another sent with it is dropped. One to 128 visible ASCII characters, with no spaces.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Dedupe(String);

impl Dedupe {
    /// The most characters a key has.
    pub const MAX: usize = 128;

    /// The key as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Dedupe {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Dedupe {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// Text that is no dedupe key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DedupeError {
    /// Empty.
    Empty,
    /// More than [`Dedupe::MAX`] characters.
    Long {
        /// How many it has.
        found: usize,
    },
    /// A character that is not visible ASCII, at this byte offset.
    Character {
        /// The byte offset.
        at: usize,
    },
}

impl fmt::Display for DedupeError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => out.write_str("a dedupe key is not empty"),
            Self::Long { found } => write!(out, "a dedupe key has at most {} characters, not {found}", Dedupe::MAX),
            Self::Character { at } => write!(out, "byte {at} of the dedupe key is not visible ASCII"),
        }
    }
}

impl std::error::Error for DedupeError {}

impl FromStr for Dedupe {
    type Err = DedupeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty() {
            return Err(DedupeError::Empty);
        }
        if text.len() > Self::MAX {
            return Err(DedupeError::Long { found: text.len() });
        }
        match text.bytes().position(|b| !b.is_ascii_graphic()) {
            Some(at) => Err(DedupeError::Character { at }),
            None => Ok(Self(text.to_owned())),
        }
    }
}

/// A message to send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// The payload, opaque to the queue.
    pub body: Vec<u8>,
    /// What makes it once, if anything.
    pub dedupe: Option<Dedupe>,
}

/// A message handed to a consumer for the length of a lease.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delivery {
    /// Which message.
    pub id: MessageId,
    /// How many times it has been handed out, this time included: one on the first delivery.
    /// A consumer that sees too high a number gives up on the message and acks it.
    pub attempt: u32,
    /// The payload.
    pub body: Vec<u8>,
}
