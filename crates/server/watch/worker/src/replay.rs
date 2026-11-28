//! A fetch that serves what it was given, and the three forges' recorded advertisements.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, PoisonError};

use crate::{Fetch, Get, Reply};

/// The replay has nothing for this URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unserved {
    /// The URL asked.
    pub url: String,
}

impl fmt::Display for Unserved {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "nothing is recorded for {}", self.url)
    }
}

impl std::error::Error for Unserved {}

/// Answers each URL with the reply it was given, and keeps every request it saw.
#[derive(Debug, Default)]
pub struct Replay {
    replies: HashMap<String, Reply>,
    seen: Mutex<Vec<Get>>,
}

impl Replay {
    /// A replay that serves nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// The same, answering `url` with `reply`.
    #[must_use]
    pub fn serving(mut self, url: impl Into<String>, reply: Reply) -> Self {
        self.replies.insert(url.into(), reply);
        self
    }

    /// Every request so far, in order.
    pub fn seen(&self) -> Vec<Get> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

impl Fetch for Replay {
    type Error = Unserved;

    fn get(&self, get: &Get) -> impl std::future::Future<Output = Result<Reply, Unserved>> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner).push(get.clone());
        std::future::ready(self.replies.get(&get.url).cloned().ok_or_else(|| Unserved { url: get.url.clone() }))
    }
}

/// A forge whose `info/refs` answer is recorded in `fixtures/`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Forge {
    /// GitHub: `octocat/Hello-World`.
    Github,
    /// Forgejo (Codeberg): `forgejo/forgejo-docs`.
    Forgejo,
    /// GitLab: `gitlab-org/gitlab-test`.
    Gitlab,
}

impl Forge {
    /// The forge a name (`github`, `forgejo`, `gitlab`) says.
    pub fn named(name: &str) -> Option<Self> {
        match name {
            "github" => Some(Self::Github),
            "forgejo" => Some(Self::Forgejo),
            "gitlab" => Some(Self::Gitlab),
            _ => None,
        }
    }

    /// The recorded answer's body.
    pub const fn body(self) -> &'static [u8] {
        match self {
            Self::Github => include_bytes!("../fixtures/github.info-refs"),
            Self::Forgejo => include_bytes!("../fixtures/forgejo.info-refs"),
            Self::Gitlab => include_bytes!("../fixtures/gitlab.info-refs"),
        }
    }

    /// The repository the answer was recorded from.
    pub const fn url(self) -> &'static str {
        match self {
            Self::Github => "https://github.com/octocat/Hello-World.git",
            Self::Forgejo => "https://codeberg.org/forgejo/forgejo-docs.git",
            Self::Gitlab => "https://gitlab.com/gitlab-org/gitlab-test.git",
        }
    }
}
