//! The key a pass is filed under.

use std::fmt;

use crate::cache::hash::{Hash, Hasher};

/// Everything a pass is keyed by.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Facts<'a> {
    pub(crate) job: &'a str,
    pub(crate) words: &'a [String],
    /// The job's own environment variables.
    pub(crate) env: &'a [String],
    pub(crate) digest: Hash,
    /// The architecture of the host, for a job whose output depends on it.
    pub(crate) arch: Option<&'a str>,
    /// The channel `rust-toolchain.toml` pins.
    pub(crate) channel: Option<&'a str>,
    /// The digest of the job file.
    pub(crate) config: Hash,
}

/// The key of a pass: the hash of its [`Facts`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Key(pub(in crate::cache) Hash);

impl Key {
    pub(crate) fn of(facts: &Facts<'_>) -> Self {
        let mut hasher = Hasher::default();
        hasher.field(b"farm-pass-1").field(facts.job.as_bytes());
        for word in facts.words {
            hasher.field(word.as_bytes());
        }
        hasher.field(b"env");
        for var in facts.env {
            hasher.field(var.as_bytes());
        }
        hasher.field(facts.digest.to_string().as_bytes());
        hasher.field(facts.arch.unwrap_or("any").as_bytes());
        hasher.field(facts.channel.unwrap_or("none").as_bytes());
        hasher.field(facts.config.to_string().as_bytes());
        Self(hasher.finish())
    }

    /// The hash, for a mark to carry.
    pub(crate) const fn hash(self) -> Hash {
        self.0
    }

    /// The key of the same job running in one workspace: what a second copy is told apart by.
    pub(crate) fn within(self, workspace: &str) -> Self {
        Self(
            Hasher::default()
                .field(b"farm-run-1")
                .field(self.0.to_string().as_bytes())
                .field(workspace.as_bytes())
                .finish(),
        )
    }
}

impl fmt::Display for Key {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::tests::facts;

    #[test]
    fn the_key_follows_each_fact() {
        let words = vec!["cargo".to_owned(), "xtask".to_owned()];
        let base = Key::of(&facts(&words, 1));
        assert_eq!(base, Key::of(&facts(&words, 1)));
        assert_ne!(base, Key::of(&facts(&words, 2)), "the digest");
        let other = vec!["cargo".to_owned(), "xtask2".to_owned()];
        assert_ne!(base, Key::of(&facts(&other, 1)), "the command");
        let mut changed = facts(&words, 1);
        changed.arch = Some("aarch64");
        assert_ne!(base, Key::of(&changed), "the host class");
        changed = facts(&words, 1);
        changed.channel = Some("1.101.0");
        assert_ne!(base, Key::of(&changed), "the toolchain");
        changed = facts(&words, 1);
        changed.config = format!("{:032x}", 9).parse().unwrap();
        assert_ne!(base, Key::of(&changed), "the job file");
        changed = facts(&words, 1);
        changed.job = "lint";
        assert_ne!(base, Key::of(&changed), "the job name");
        assert_ne!(base.within("a"), base.within("b"));
    }
}
