//! What the binary was built from, and whether the checkout has moved on since: `farm
//! --version`, and the warning of `farm status`.

use std::fmt;
use std::path::Path;

use crate::cli::digest::Digest;

/// The crate's directory below a workspace root.
const CRATE: [&str; 2] = ["tools", "farm"];

/// The binary's version and the hash of the source it was built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Version {
    number: &'static str,
    source: &'static str,
}

impl Version {
    /// This binary's.
    pub(crate) const fn current() -> Self {
        Self { number: env!("CARGO_PKG_VERSION"), source: env!("FARM_SOURCE_HASH") }
    }

    /// Whether the farm source of the workspace at `root` is the one this binary was built from.
    pub(crate) fn against(&self, root: &Path) -> Freshness {
        let dir = CRATE.iter().fold(root.to_path_buf(), |dir, part| dir.join(part));
        if !dir.is_dir() {
            return Freshness::Absent;
        }
        match Digest::of(&dir) {
            Ok(now) if now.to_string() == self.source => Freshness::Current,
            Ok(now) => Freshness::Stale { built: self.source, now },
            Err(source) => Freshness::Unreadable(source),
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "farm {} (source {})", self.number, self.source)
    }
}

/// How the installed binary stands against a checkout.
#[derive(Debug)]
pub(crate) enum Freshness {
    Current,
    /// The checkout's farm source differs from the binary's.
    Stale {
        built: &'static str,
        now: Digest,
    },
    /// The checkout has no farm source.
    Absent,
    Unreadable(std::io::Error),
}

impl fmt::Display for Freshness {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Current => write!(out, "the installed farm matches this checkout"),
            Self::Stale { built, now } => write!(
                out,
                "the installed farm (source {built}) is not this checkout's (source {now}); run `cargo install --path tools/farm --locked`"
            ),
            Self::Absent => write!(out, "this checkout has no tools/farm to compare the installed farm with"),
            Self::Unreadable(_) => {
                write!(out, "this checkout's tools/farm could not be read to compare it with the installed farm")
            }
        }
    }
}

impl std::error::Error for Freshness {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreadable(source) => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_checkout_of_this_source_is_current_and_an_edit_makes_it_stale() {
        let root = std::env::temp_dir().join(format!("farm-version-{}", std::process::id()));
        let dir = root.join("tools").join("farm");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.rs"), "one").unwrap();
        let built = Digest::of(&dir).unwrap().to_string();
        let version = Version { number: "0.0.0", source: Box::leak(built.into_boxed_str()) };
        assert!(matches!(version.against(&root), Freshness::Current));
        std::fs::write(dir.join("a.rs"), "two").unwrap();
        let stale = version.against(&root);
        assert!(matches!(stale, Freshness::Stale { .. }), "{stale}");
        assert!(stale.to_string().contains("cargo install --path tools/farm --locked"));
        assert!(matches!(version.against(&root.join("none")), Freshness::Absent));
        std::fs::remove_dir_all(&root).unwrap();
    }
}
