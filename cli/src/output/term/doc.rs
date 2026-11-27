//! A document is lines of pieces: a role and some text. It knows its glyph set and width and
//! nothing of how a terminal paints.

use anstream::adapter::strip_str;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::glyph::{Glyph, Glyphs};
use super::layout::Cut;
use super::look::Look;
use super::role::{Role, Strength};

/// A block of output: it produces roles and text, never escapes.
pub(crate) trait Render {
    /// Pushes this block's lines into `into`.
    fn render(&self, into: &mut Doc);
}

impl<R: Render + ?Sized> Render for &R {
    fn render(&self, into: &mut Doc) {
        (**self).render(into);
    }
}

/// Text from a file or a path made safe to print: a control character, which could move the
/// cursor or set a title, is replaced.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Safe<'a>(pub(crate) &'a str);

impl Safe<'_> {
    /// The text with each control character written as U+FFFD.
    pub(crate) fn text(self) -> String {
        self.0.chars().map(|c| if c.is_control() { '\u{fffd}' } else { c }).collect()
    }

    /// The text with each control character other than a newline, a tab or a carriage return
    /// written as `?` for each of its bytes, so a byte offset into it stays an offset into the
    /// original.
    pub(crate) fn offsets(self) -> String {
        let mut safe = String::with_capacity(self.0.len());
        for c in self.0.chars() {
            if c.is_control() && !matches!(c, '\n' | '\t' | '\r') {
                safe.extend(std::iter::repeat_n('?', c.len_utf8()));
            } else {
                safe.push(c);
            }
        }
        safe
    }
}

/// One run of text in one role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Piece {
    pub(crate) role: Role,
    pub(crate) strength: Strength,
    pub(crate) text: String,
    /// The text already carries its own escape codes, as a snippet from `annotate-snippets` does.
    pub(crate) styled: bool,
}

impl Piece {
    fn new(role: Role, strength: Strength, text: &str) -> Self {
        Self { role, strength, text: Safe(text).text(), styled: false }
    }

    /// Text that was styled by someone else; its width is that of the text without the escapes.
    pub(crate) fn ansi(text: String) -> Self {
        Self { role: Role::Plain, strength: Strength::Normal, text, styled: true }
    }

    /// Columns the text takes.
    pub(crate) fn width(&self) -> usize {
        if self.styled { strip_str(&self.text).to_string().width() } else { self.text.width() }
    }

    /// The text as a reader sees it, with no escapes.
    pub(crate) fn plain(&self) -> String {
        if self.styled { strip_str(&self.text).to_string() } else { self.text.clone() }
    }
}

/// One line: pieces side by side.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Line(Vec<Piece>);

impl Line {
    /// An empty line.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// `text` in `role`, after what is there.
    #[must_use]
    pub(crate) fn with(mut self, role: Role, text: impl AsRef<str>) -> Self {
        self.0.push(Piece::new(role, Strength::Normal, text.as_ref()));
        self
    }

    /// `text` in `role`, bold, after what is there.
    #[must_use]
    pub(crate) fn strong(mut self, role: Role, text: impl AsRef<str>) -> Self {
        self.0.push(Piece::new(role, Strength::Strong, text.as_ref()));
        self
    }

    /// `text` in the plain role.
    pub(crate) fn plain(text: impl AsRef<str>) -> Self {
        Self::new().with(Role::Plain, text)
    }

    /// `text` in `role`, where what sits between backticks is something to type and is written in
    /// the literal role. The backticks stay, so the text reads the same without colour.
    pub(crate) fn marked(role: Role, text: &str) -> Self {
        let parts: Vec<&str> = text.split('`').collect();
        if parts.len().is_multiple_of(2) {
            return Self::new().with(role, text);
        }
        let mut line = Self::new();
        for (at, part) in parts.into_iter().enumerate() {
            if at % 2 == 1 {
                line = line.with(role, "`").with(Role::Literal, part).with(role, "`");
            } else {
                line = line.with(role, part);
            }
        }
        line
    }

    /// Puts a piece after what is there.
    pub(crate) fn push(&mut self, piece: Piece) {
        self.0.push(piece);
    }

    /// Puts `other` after what is there.
    #[must_use]
    pub(crate) fn then(mut self, other: Self) -> Self {
        self.0.extend(other.0);
        self
    }

    /// `count` spaces after what is there.
    #[must_use]
    pub(crate) fn pad(self, count: usize) -> Self {
        self.with(Role::Plain, " ".repeat(count))
    }

    /// The pieces.
    pub(crate) fn pieces(&self) -> &[Piece] {
        &self.0
    }

    /// Whether nothing would be written.
    pub(crate) fn is_empty(&self) -> bool {
        self.0.iter().all(|piece| piece.text.is_empty())
    }

    /// Columns the line takes, escapes not counted.
    pub(crate) fn width(&self) -> usize {
        self.0.iter().map(Piece::width).sum()
    }

    /// The text as a reader sees it, with no styling.
    pub(crate) fn text(&self) -> String {
        self.0.iter().map(Piece::plain).collect()
    }

    /// The line cut to `room` columns with a mark of the cut: in its middle, which keeps both ends
    /// of a path, or at its end. A line that fits is returned as it is.
    pub(crate) fn cut(&self, room: usize, how: Cut, glyphs: Glyphs) -> Self {
        if self.width() <= room {
            return self.clone();
        }
        let dots = glyphs.get(Glyph::Truncation);
        let role = self.0.first().map_or(Role::Plain, |piece| piece.role);
        let text = self.text();
        let keep = room.saturating_sub(dots.width());
        let marked = Self::new().with(Glyph::Truncation.role(), dots);
        match how {
            Cut::End => Self::new().with(role, Slice(&text).head(keep)).then(marked),
            Cut::Middle => {
                let tail = keep / 2;
                let head = keep - tail;
                Self::new().with(role, Slice(&text).head(head)).then(marked).with(role, Slice(&text).tail(tail))
            }
        }
    }
}

/// A piece of text taken by columns.
#[derive(Clone, Copy, Debug)]
struct Slice<'a>(&'a str);

impl<'a> Slice<'a> {
    /// The longest start of the text that fits `room` columns.
    fn head(self, room: usize) -> &'a str {
        let mut used = 0;
        for (at, c) in self.0.char_indices() {
            used += c.width().unwrap_or(0);
            if used > room {
                return self.0.get(..at).unwrap_or_default();
            }
        }
        self.0
    }

    /// The longest end of the text that fits `room` columns.
    fn tail(self, room: usize) -> &'a str {
        let mut used = 0;
        for (at, c) in self.0.char_indices().rev() {
            used += c.width().unwrap_or(0);
            if used > room {
                return self.0.get(at + c.len_utf8()..).unwrap_or_default();
            }
        }
        self.0
    }
}

/// The lines a block produced, and how they are to be laid out.
#[derive(Clone, Debug)]
pub(crate) struct Doc {
    look: Look,
    lines: Vec<Line>,
}

impl Doc {
    /// An empty document for output that looks like `look`.
    pub(crate) fn new(look: Look) -> Self {
        Self { look, lines: Vec::new() }
    }

    /// How the output looks.
    pub(crate) const fn look(&self) -> Look {
        self.look
    }

    /// Columns a line may take, or all of them when the width is not limited.
    pub(crate) const fn room(&self) -> usize {
        self.look.width.room()
    }

    /// Adds a line.
    pub(crate) fn push(&mut self, line: Line) {
        self.lines.push(line);
    }

    /// Adds an empty line.
    pub(crate) fn blank(&mut self) {
        self.lines.push(Line::new());
    }

    /// `glyph` as a line's start, in its own role.
    pub(crate) fn glyph(&self, glyph: Glyph) -> Line {
        Line::new().with(glyph.role(), self.look.glyphs.get(glyph))
    }

    /// The lines.
    pub(crate) fn lines(&self) -> &[Line] {
        &self.lines
    }
}
