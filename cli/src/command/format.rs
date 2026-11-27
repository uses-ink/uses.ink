//! The formats `ink` reads a site's markdown with, as the engine's [`Format`]: the markdown
//! format, or Obsidian's in a vault. `site.format` says which: a pin in `ink.toml`, else the
//! `.obsidian/` at the folder's top that makes it a vault, as `.git` makes it a repository, else
//! markdown. The engine decides it ([`engine::Chosen`]); this is the format it names.

use engine::Format;
use model::record::PluginId;
use model::{ContentPath, Read};

use crate::error::Error;

/// The format a site's markdown is read with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Native {
    /// `ink:format-markdown`.
    Markdown,
    /// `ink:format-obsidian`, for a vault.
    Obsidian,
}

impl Native {
    /// The format's plugin id.
    pub(crate) const fn id(self) -> &'static str {
        match self {
            Self::Markdown => "ink:format-markdown",
            Self::Obsidian => "ink:format-obsidian",
        }
    }
}

impl TryFrom<&PluginId> for Native {
    type Error = Error;

    /// The bundled format the id names; a format this ink does not hold is refused.
    fn try_from(id: &PluginId) -> Result<Self, Error> {
        [Self::Markdown, Self::Obsidian]
            .into_iter()
            .find(|native| native.id() == id.to_string())
            .ok_or_else(|| Error::Format { id: id.clone() })
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
