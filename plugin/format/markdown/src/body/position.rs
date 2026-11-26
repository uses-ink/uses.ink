//! Where comrak's nodes are in the file: comrak counts lines and columns in the body, a span
//! counts bytes in the file.

use comrak::nodes::Sourcepos;
use model::Span;

use crate::html::Piece;

/// The places of one body in its file.
#[derive(Debug)]
pub(crate) struct Positions<'a> {
    file: &'a str,
    /// The whole file, where a position cannot be placed more precisely.
    whole: Span,
    /// Where the body starts in the file.
    base: usize,
    /// Where each line of the body starts, from the body's start.
    lines: Vec<usize>,
}

impl<'a> Positions<'a> {
    /// The places of the body of `file` starting at byte `base`. comrak ends a line at `\n`,
    /// `\r\n` or a lone `\r`.
    pub(crate) fn new(file: &'a str, base: usize) -> Option<Self> {
        let whole = Span::new(0, file.len()).ok()?;
        let bytes = file.get(base..)?.as_bytes();
        let mut lines = vec![0];
        for (at, &byte) in bytes.iter().enumerate() {
            if byte == b'\n' || (byte == b'\r' && bytes.get(at + 1) != Some(&b'\n')) {
                lines.push(at + 1);
            }
        }
        Some(Self { file, whole, base, lines })
    }

    /// A comrak position (1-based lines, 1-based byte columns, inclusive end) as a span of the
    /// file.
    pub(crate) fn span(&self, position: Sourcepos) -> Span {
        let offset = |line: usize, column: usize| {
            let start = self.lines.get(line.saturating_sub(1)).map_or(self.file.len(), |&s| self.base + s);
            (start + column.saturating_sub(1)).min(self.file.len())
        };
        let start = offset(position.start.line, position.start.column);
        let end = (offset(position.end.line, position.end.column) + 1).min(self.file.len()).max(start);
        Span::new(start, end).unwrap_or(self.whole)
    }

    /// Where raw HTML at `span` is in the file: written there as is, or reshaped by comrak
    /// (container prefixes taken out of its lines).
    pub(crate) fn placement(&self, html: &str, span: Span) -> Placement {
        let exact = self.file.get(span.start()..).is_some_and(|rest| rest.starts_with(html));
        if exact { Placement::Exact(span) } else { Placement::Within(span) }
    }
}

/// Where a piece of raw HTML is in the file.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Placement {
    /// The HTML is the file's text from this span's start, so a tag's offset in it is exact.
    Exact(Span),
    /// The HTML is somewhere in this span.
    Within(Span),
}

impl Placement {
    /// Where `piece` is: exactly, for a tag whose place is exact.
    pub(crate) fn at(self, piece: &Piece<'_>) -> Span {
        match (self, piece.tag()) {
            (Self::Exact(span), Some(tag)) => {
                Span::new(span.start() + tag.start, span.start() + tag.end).unwrap_or(span)
            }
            (Self::Exact(span) | Self::Within(span), _) => span,
        }
    }
}
