//! The markdown format as the engine's [`Format`].

use engine::Format;
use model::{ContentPath, Read};

/// `.md` and `.markdown` files, read by `ink:format-markdown`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Markdown;

impl Format for Markdown {
    fn handles(&self, path: &ContentPath) -> bool {
        path.extension().is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "md" | "markdown"))
    }

    fn read(&self, path: &ContentPath, bytes: &[u8]) -> Read {
        format_markdown::read(path, bytes)
    }
}
