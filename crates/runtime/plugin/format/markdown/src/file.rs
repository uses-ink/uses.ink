//! One file as text, cut into its frontmatter block and its body.

use model::Span;

use crate::error::Encoding;

const BYTE_ORDER_MARK: &str = "\u{feff}";

/// A markdown file: UTF-8 text, where its frontmatter block is, and where its body starts.
#[derive(Debug)]
pub(crate) struct File<'a> {
    /// The whole file, byte order mark included, so every span is the file's.
    pub(crate) text: &'a str,
    pub(crate) front: Option<Fence>,
    /// Where the body starts.
    pub(crate) body: usize,
}

/// A frontmatter block, as its fences place it.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Fence {
    /// The YAML between the fences, and the whole block with its fences.
    Closed { yaml: Span, block: Span },
    /// An opening `---` with no closing line: the opening line.
    Unclosed { opening: Span },
}

impl<'a> TryFrom<&'a [u8]> for File<'a> {
    type Error = Encoding;

    /// The file, with a frontmatter block when it starts with a `---` line (after a byte order
    /// mark), and that block ends at the next `---` or `...` line.
    fn try_from(bytes: &'a [u8]) -> Result<Self, Encoding> {
        let text = std::str::from_utf8(bytes).map_err(|source| Encoding { source, len: bytes.len() })?;
        let base = if text.starts_with(BYTE_ORDER_MARK) { BYTE_ORDER_MARK.len() } else { 0 };
        let mut lines = Lines { text, at: base };
        let unfenced = Self { text, front: None, body: base };
        let Some(first) = lines.next().filter(|line| line.is("---")) else { return Ok(unfenced) };
        let opening = first.span;
        for line in lines.filter(|line| line.is("---") || line.is("...")) {
            let yaml = Span::new(opening.end(), line.span.start());
            let block = Span::new(opening.start(), line.span.end());
            if let (Ok(yaml), Ok(block)) = (yaml, block) {
                return Ok(Self { front: Some(Fence::Closed { yaml, block }), body: line.span.end(), ..unfenced });
            }
        }
        Ok(Self { front: Some(Fence::Unclosed { opening }), ..unfenced })
    }
}

/// One line of the file and where it is, its line ending included in the span.
#[derive(Debug)]
struct Line<'a> {
    text: &'a str,
    span: Span,
}

impl Line<'_> {
    /// Whether the line is `fence`, with nothing after it but spaces, tabs and the line ending.
    fn is(&self, fence: &str) -> bool {
        self.text.trim_end_matches(['\n', '\r', ' ', '\t']) == fence
    }
}

/// The lines of a text from a byte offset.
#[derive(Debug)]
struct Lines<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Iterator for Lines<'a> {
    type Item = Line<'a>;

    fn next(&mut self) -> Option<Line<'a>> {
        let rest = self.text.get(self.at..).filter(|rest| !rest.is_empty())?;
        let len = rest.find('\n').map_or(rest.len(), |end| end + 1);
        let text = rest.get(..len)?;
        let span = Span::new(self.at, self.at + len).ok()?;
        self.at += len;
        Some(Line { text, span })
    }
}
