//! The format a site's markdown is read with.

use engine::Format;
use model::record::PluginId;
use model::{ContentPath, Read};

/// `ink:format-markdown`, or `ink:format-obsidian` in a vault: the formats compiled in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reader {
    Markdown,
    Obsidian,
}

impl Reader {
    /// The format's plugin id.
    const fn id(self) -> &'static str {
        match self {
            Self::Markdown => "ink:format-markdown",
            Self::Obsidian => "ink:format-obsidian",
        }
    }
}

impl TryFrom<&PluginId> for Reader {
    /// The id of a format this builder does not hold.
    type Error = PluginId;

    fn try_from(id: &PluginId) -> Result<Self, PluginId> {
        [Self::Markdown, Self::Obsidian].into_iter().find(|reader| reader.id() == id.to_string()).ok_or_else(|| id.clone())
    }
}

impl Format for Reader {
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
