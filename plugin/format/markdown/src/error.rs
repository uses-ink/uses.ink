//! Why part of a file was not read as written, and the one sink every part of the format reports
//! into. An [`Error`] keeps its cause typed; it becomes a catalog [`Problem`] only in
//! [`Report::refuse`], the edge of the format.

use std::fmt;

use model::{Diagnostic, Expected, Notice, Problem, Span, Subject};

/// Why part of a file was not read as written.
#[derive(Debug)]
pub(crate) enum Error {
    /// The file is not UTF-8.
    Encoding(Encoding),
    /// An opening `---` with no closing line.
    Unclosed,
    /// The YAML does not parse, or is not a mapping of keys.
    Yaml {
        /// What the YAML parser refused.
        source: serde_saphyr::Error,
        /// The lines of the file before the YAML, so the line the parser counts from the
        /// block is shown as the file's.
        before: u64,
    },
    /// A key the engine reads holds a value it cannot use.
    Value {
        /// The key as the format spells it.
        key: &'static str,
        /// What the value had to be.
        source: Expected,
    },
    /// A key sets what an earlier key already set.
    Conflict {
        /// The later key, ignored.
        key: &'static str,
        /// The earlier key, kept.
        other: &'static str,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encoding(encoding) => fmt::Display::fmt(encoding, out),
            Self::Unclosed => out.write_str("it has no closing `---` line, so the whole file is read as the body"),
            Self::Yaml { source, before } => {
                let mut render = serde_saphyr::RenderOptions::new(&serde_saphyr::UserMessageFormatter);
                render.snippets = serde_saphyr::SnippetMode::Off;
                render.line_offset = *before;
                source.render_to(out, render)
            }
            Self::Value { key, source } => write!(out, "`{key}` takes {source}"),
            Self::Conflict { key, other } => write!(out, "`{key}` sets the same thing as `{other}` before it"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encoding(encoding) => Some(encoding),
            Self::Yaml { source, .. } => Some(source),
            Self::Value { source, .. } => Some(source),
            Self::Unclosed | Self::Conflict { .. } => None,
        }
    }
}

impl From<Error> for Problem {
    /// The catalog problem the error is. A YAML error's text is its reason: the catalog field
    /// is text, and the parser's message is the one place its cause is written.
    fn from(error: Error) -> Self {
        match error {
            Error::Encoding(encoding) => Self::SourceFileEncoding { at: encoding.source.valid_up_to() as u64 },
            Error::Unclosed | Error::Yaml { .. } => Self::FrontmatterSyntaxInvalid { reason: error.to_string() },
            Error::Value { key, source } => {
                Self::FrontmatterValueInvalid { key: key.to_owned(), expected: source.to_string() }
            }
            Error::Conflict { key, other } => {
                Self::FrontmatterKeyConflict { key: key.to_owned(), other: other.to_owned() }
            }
        }
    }
}

/// A file that is not UTF-8 text.
#[derive(Debug)]
pub(crate) struct Encoding {
    /// Where the first byte that is not UTF-8 is.
    pub(crate) source: std::str::Utf8Error,
    /// The file's length.
    pub(crate) len: usize,
}

impl Encoding {
    /// The first byte that is not UTF-8.
    pub(crate) fn span(&self) -> Option<Span> {
        let at = self.source.valid_up_to();
        Span::new(at, self.len.min(at + 1)).ok()
    }
}

impl fmt::Display for Encoding {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "the file is not UTF-8 from byte {}", self.source.valid_up_to())
    }
}

impl std::error::Error for Encoding {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl From<Encoding> for Error {
    fn from(encoding: Encoding) -> Self {
        Self::Encoding(encoding)
    }
}

/// What one file's read reports: its diagnostics and notices, in the order they arise, each on
/// the file.
#[derive(Debug)]
pub(crate) struct Report {
    subject: Subject,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) notices: Vec<Notice>,
}

impl Report {
    /// An empty report on `subject`.
    pub(crate) const fn new(subject: Subject) -> Self {
        Self { subject, diagnostics: Vec::new(), notices: Vec::new() }
    }

    /// Reports `error`, at `span` when it has a place.
    pub(crate) fn refuse(&mut self, error: Error, span: impl Into<Option<Span>>) {
        let mut diagnostic = Diagnostic::new(error.into()).on(self.subject.clone());
        if let Some(span) = span.into() {
            diagnostic = diagnostic.at(span);
        }
        self.diagnostics.push(diagnostic);
    }
}
