//! The formats `ink` reads a site's markdown with, as the engine's [`Format`]: the markdown
//! format, or Obsidian's in a vault. The folder says which: one holding `.obsidian/` at its top
//! is a vault, as `.git` makes it a repository, and the choice is said
//! (`format.obsidian.chosen`).

use engine::Format;
use model::{Announcement, ContentPath, Notice, Read, Snapshot};

/// The format a site's markdown is read with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Native {
    /// `ink:format-markdown`.
    Markdown,
    /// `ink:format-obsidian`, for a vault.
    Obsidian,
}

impl Native {
    /// The folder at the top of a snapshot that makes it a vault.
    const VAULT: &'static str = ".obsidian";

    /// The format's plugin id.
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Markdown => "ink:format-markdown",
            Self::Obsidian => "ink:format-obsidian",
        }
    }

    /// The file of its component, as `cargo xtask component` names it.
    pub(crate) const fn file(self) -> &'static str {
        match self {
            Self::Markdown => "format-markdown.wasm",
            Self::Obsidian => "format-obsidian.wasm",
        }
    }

    /// What choosing it says: a vault's format is announced, markdown is the default.
    pub(crate) fn notice(self) -> Option<Notice> {
        (self == Self::Obsidian).then(|| Notice::new(Announcement::FormatObsidianChosen))
    }
}

impl From<&Snapshot> for Native {
    /// Obsidian's when the snapshot holds `.obsidian/` at its top, else markdown.
    fn from(snapshot: &Snapshot) -> Self {
        let vault = snapshot.tree().iter().any(|entry| entry.path.segments().next() == Some(Self::VAULT));
        if vault { Self::Obsidian } else { Self::Markdown }
    }
}

impl Format for Native {
    fn handles(&self, path: &ContentPath) -> bool {
        match self {
            Self::Markdown => format_markdown::handles(path),
            Self::Obsidian => format_obsidian::handles(path),
        }
    }

    fn read(&self, path: &ContentPath, bytes: &[u8]) -> Read {
        match self {
            Self::Markdown => format_markdown::read(path, bytes),
            Self::Obsidian => format_obsidian::read(path, bytes),
        }
    }
}
