//! What crosses between the store and the object.

use edge_api::{Claimed, Claiming};
use edge_object_nonce::{Ask, Verdict};
use serde::{Deserialize, Serialize};

/// A claim, as the object receives it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wire {
    /// The key's 16 bytes.
    pub key: [u8; 16],
    /// The nonce's 16 bytes.
    pub nonce: [u8; 16],
    /// When the claim ends.
    pub until: u64,
    /// When it is made.
    pub now: u64,
    /// How many nonces one key may hold.
    pub most: usize,
}

impl From<Claiming> for Wire {
    fn from(claiming: Claiming) -> Self {
        Self {
            key: *claiming.key.bytes(),
            nonce: *claiming.nonce.bytes(),
            until: claiming.until,
            now: claiming.now,
            most: claiming.most,
        }
    }
}

impl From<Wire> for Ask {
    fn from(wire: Wire) -> Self {
        Self { key: wire.key, nonce: wire.nonce, until: wire.until, now: wire.now, most: wire.most }
    }
}

/// The object's answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// New: remembered.
    Fresh,
    /// Seen before.
    Replay,
    /// The key is at its bound.
    Full,
    /// The object is new and the request older than its quiet period.
    Quiet {
        /// Seconds until the quiet period ends.
        seconds: u64,
    },
}

impl From<Verdict> for Answer {
    fn from(verdict: Verdict) -> Self {
        match verdict {
            Verdict::Fresh => Self::Fresh,
            Verdict::Replay => Self::Replay,
            Verdict::Full => Self::Full,
            Verdict::Quiet { seconds } => Self::Quiet { seconds },
        }
    }
}

impl From<Answer> for Claimed {
    fn from(answer: Answer) -> Self {
        match answer {
            Answer::Fresh => Self::Fresh,
            Answer::Replay => Self::Replay,
            Answer::Full => Self::Full,
            Answer::Quiet { seconds } => Self::Quiet { seconds },
        }
    }
}
