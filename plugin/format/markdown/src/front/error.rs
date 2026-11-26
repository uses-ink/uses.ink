//! Why part of a file was not read. Each converts into a catalog [`Problem`] at the edge of the
//! format, where its `Display` becomes the problem's reason.

use std::fmt;

use model::Problem;

/// Why a frontmatter block was not read.
#[derive(Debug)]
pub(crate) enum Error {
    /// An opening `---` with no closing line.
    Unclosed,
    /// The YAML does not parse, or is not a mapping of keys. `before` is the number of lines in
    /// the file before the YAML, so the line in the message is the file's.
    Yaml { source: serde_saphyr::Error, before: u64 },
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unclosed => out.write_str("it has no closing `---` line, so the whole file is read as the body"),
            Self::Yaml { source, before } => {
                let mut render = serde_saphyr::RenderOptions::new(&serde_saphyr::UserMessageFormatter);
                render.snippets = serde_saphyr::SnippetMode::Off;
                render.line_offset = *before;
                source.render_to(out, render)
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unclosed => None,
            Self::Yaml { source, .. } => Some(source),
        }
    }
}

impl From<&Error> for Problem {
    fn from(error: &Error) -> Self {
        Self::FrontmatterSyntaxInvalid { reason: error.to_string() }
    }
}
