//! Text that wraps to the width of the document: a paragraph, a list of terms and what they mean,
//! a list of lines to type with their comments. Like the other layout types these are the only
//! code that breaks a line (`docs/cli.md` section 6): a piece that is marked whole (a default, a
//! variable) moves to the next line instead of being split, and a line that wraps hangs under its
//! first word.

use crate::output::term::{Doc, Line, Piece, Render, Role};

/// What is between two spaces: pieces that wrapping does not part.
#[derive(Clone, Debug, Default)]
struct Word {
    pieces: Vec<Piece>,
}

impl Word {
    fn width(&self) -> usize {
        self.pieces.iter().map(Piece::width).sum()
    }

    fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    /// The word as words of its own, when it is a whole piece that is wider than the line: it is
    /// then broken between its words like any text.
    fn loosened(self) -> Vec<Self> {
        let mut words = Vec::new();
        let mut word = Self::default();
        for piece in self.pieces {
            if !piece.whole {
                word.pieces.push(piece);
                continue;
            }
            for (at, part) in piece.text.split(' ').enumerate() {
                if at > 0 && !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
                if !part.is_empty() {
                    word.pieces.push(Piece { text: part.to_owned(), whole: false, ..piece.clone() });
                }
            }
        }
        if !word.is_empty() {
            words.push(word);
        }
        words
    }
}

impl Line {
    /// The words of the line: text between spaces, and each whole or styled piece as it is.
    fn words(&self) -> Vec<Word> {
        let mut words = Vec::new();
        let mut word = Word::default();
        for piece in self.pieces() {
            if piece.whole || piece.styled {
                word.pieces.push(piece.clone());
                continue;
            }
            for (at, part) in piece.text.split(' ').enumerate() {
                if at > 0 && !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
                if !part.is_empty() {
                    word.pieces.push(Piece { text: part.to_owned(), ..piece.clone() });
                }
            }
        }
        if !word.is_empty() {
            words.push(word);
        }
        words
    }

    /// The line as lines of at most `first` columns for the first and `rest` for the others, broken
    /// between words. A word wider than its line is kept whole and overflows.
    pub(crate) fn wrap(&self, first: usize, rest: usize) -> Vec<Self> {
        let mut lines = Vec::new();
        let mut line = Self::new();
        let (mut used, mut room) = (0, first.max(1));
        let mut pending = self.words();
        pending.reverse();
        while let Some(word) = pending.pop() {
            let width = word.width();
            if width > room.max(rest.max(1)) && word.pieces.iter().any(|piece| piece.whole) {
                let parts = word.loosened();
                if parts.len() > 1 {
                    pending.extend(parts.into_iter().rev());
                    continue;
                }
                let Some(only) = parts.into_iter().next() else { continue };
                pending.push(Word {
                    pieces: only.pieces.into_iter().map(|piece| Piece { whole: false, ..piece }).collect(),
                });
                continue;
            }
            if used > 0 && used + 1 + width > room {
                lines.push(std::mem::take(&mut line));
                (used, room) = (0, rest.max(1));
            }
            if used > 0 {
                line = line.pad(1);
                used += 1;
            }
            for piece in word.pieces {
                line.push(piece);
            }
            used += width;
        }
        if !line.is_empty() || lines.is_empty() {
            lines.push(line);
        }
        lines
    }
}

/// A paragraph, wrapped to the width and indented.
#[derive(Clone, Debug)]
pub(crate) struct Paragraph {
    text: Line,
    indent: usize,
}

impl Paragraph {
    /// `text`, indented by `indent` columns.
    pub(crate) const fn new(text: Line, indent: usize) -> Self {
        Self { text, indent }
    }
}

impl Render for Paragraph {
    fn render(&self, into: &mut Doc) {
        let room = into.room().saturating_sub(self.indent);
        for line in self.text.wrap(room, room) {
            into.push(Line::new().pad(self.indent).then(line));
        }
    }
}

/// Terms and what each means: the options of a command, its subcommands, the exit codes. The
/// terms line up in one column and the meanings wrap beside them; when a term is wider than
/// 45 percent of the line, or the meanings would get fewer than 34 columns, the meaning is written
/// below its term instead.
#[derive(Clone, Debug, Default)]
pub(crate) struct Defs {
    rows: Vec<(Line, Line)>,
    indent: usize,
    inline: bool,
}

impl Defs {
    /// Columns between a term and its meaning.
    const GAP: usize = 2;
    /// How far a meaning below its term is indented past it.
    const HANG: usize = 6;
    /// The fewest columns a meaning beside its term may have.
    const LEAST: usize = 34;

    /// A list indented by `indent` columns.
    pub(crate) fn new(indent: usize) -> Self {
        Self { rows: Vec::new(), indent, inline: false }
    }

    /// A list whose meanings stay beside their terms however narrow the line is, such as a record.
    pub(crate) fn inline(indent: usize) -> Self {
        Self { rows: Vec::new(), indent, inline: true }
    }

    /// One term and its meaning.
    #[must_use]
    pub(crate) fn row(mut self, term: Line, meaning: Line) -> Self {
        self.rows.push((term, meaning));
        self
    }
}

impl Render for Defs {
    fn render(&self, into: &mut Doc) {
        let limit = into.room().saturating_sub(self.indent);
        let cap = if limit == usize::MAX { limit } else { limit.saturating_mul(9) / 20 };
        let column = self.rows.iter().map(|(term, _)| term.width()).filter(|width| *width <= cap).max().unwrap_or(0);
        let beside = limit.saturating_sub(column + Self::GAP);
        let stacked_all = beside < Self::LEAST && !self.inline;
        for (term, meaning) in &self.rows {
            let lead = Line::new().pad(self.indent);
            if meaning.is_empty() {
                into.push(lead.then(term.clone()));
            } else if stacked_all || (term.width() > cap && !self.inline) {
                into.push(lead.then(term.clone()));
                let room = limit.saturating_sub(Self::HANG);
                for line in meaning.wrap(room, room) {
                    into.push(Line::new().pad(self.indent + Self::HANG).then(line));
                }
            } else {
                let mut lines = meaning.wrap(beside, beside).into_iter();
                let first = lines.next().unwrap_or_default();
                let fill = column.saturating_sub(term.width()) + Self::GAP;
                into.push(lead.then(term.clone()).pad(fill).then(first));
                for line in lines {
                    into.push(Line::new().pad(self.indent + column + Self::GAP).then(line));
                }
            }
        }
    }
}

/// Lines to type, each with what it does. Beside their commands, the comments line up when every
/// one fits; otherwise each comment is written above its command, so a command is never split.
#[derive(Clone, Debug, Default)]
pub(crate) struct Samples {
    rows: Vec<(Line, Line)>,
    indent: usize,
}

impl Samples {
    /// Columns between a command and its comment.
    const GAP: usize = 2;

    /// A list indented by `indent` columns.
    pub(crate) fn new(indent: usize) -> Self {
        Self { rows: Vec::new(), indent }
    }

    /// One command and its comment.
    #[must_use]
    pub(crate) fn row(mut self, command: Line, comment: Line) -> Self {
        self.rows.push((command, comment));
        self
    }
}

impl Render for Samples {
    fn render(&self, into: &mut Doc) {
        let limit = into.room().saturating_sub(self.indent);
        let column = self.rows.iter().map(|(command, _)| command.width()).max().unwrap_or(0);
        let fits = self.rows.iter().all(|(_, comment)| column + Self::GAP + 2 + comment.width() <= limit);
        let remark = |comment: &Line| Line::new().with(Role::Quiet, "# ").then(comment.clone());
        for (at, (command, comment)) in self.rows.iter().enumerate() {
            let lead = Line::new().pad(self.indent);
            if comment.is_empty() {
                into.push(lead.then(command.clone()));
            } else if fits {
                let fill = column.saturating_sub(command.width()) + Self::GAP;
                into.push(lead.then(command.clone()).pad(fill).then(remark(comment)));
            } else {
                if at > 0 {
                    into.blank();
                }
                let room = limit.saturating_sub(2);
                for line in comment.wrap(room, room) {
                    into.push(lead.clone().then(Line::new().with(Role::Quiet, "# ")).then(line));
                }
                into.push(lead.then(command.clone()));
            }
        }
    }
}

/// A document that was laid out already, such as a page of help written at a width of its own:
/// its lines are written as they are.
#[derive(Clone, Debug)]
pub(crate) struct Finished(pub(crate) Doc);

impl Render for Finished {
    fn render(&self, into: &mut Doc) {
        for line in self.0.lines() {
            into.push(line.clone());
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::output::term::Look;

    fn text(block: &impl Render, width: usize) -> String {
        let mut doc = Doc::new(Look { width: crate::output::term::Width::Columns(width), ..Look::PLAIN });
        block.render(&mut doc);
        doc.text()
    }

    #[test]
    fn a_paragraph_breaks_between_words_and_keeps_an_atom_whole() {
        let line = Line::plain("one two three ").atom(Role::Quiet, "[default: a b]").with(Role::Plain, " tail");
        assert_eq!(text(&Paragraph::new(line, 2), 20), "  one two three\n  [default: a b]\n  tail\n");
    }

    #[test]
    fn terms_line_up_and_meanings_hang_under_themselves() {
        let defs = Defs::new(2).row(Line::plain("-a"), Line::plain("short one")).row(
            Line::plain("--long <V>"),
            Line::plain("a meaning that wraps onto a second and a third line of its own"),
        );
        let out = text(&defs, 60);
        assert!(
            out.starts_with("  -a          short one\n  --long <V>  a meaning that wraps onto a second and a third\n"),
            "{out}"
        );
        assert!(out.lines().skip(2).all(|line| line.starts_with("              ")), "{out}");
    }

    #[test]
    fn a_narrow_page_writes_each_meaning_below_its_term() {
        let defs = Defs::new(2).row(Line::plain("--source <KIND>"), Line::plain("what to read, folder or git"));
        assert_eq!(text(&defs, 40), "  --source <KIND>\n        what to read, folder or git\n");
    }

    #[test]
    fn samples_put_the_comment_above_when_they_do_not_fit_beside() {
        let samples = Samples::new(2)
            .row(Line::plain("ink check"), Line::plain("read it"))
            .row(Line::plain("ink build -o public"), Line::plain("build into ./public"));
        assert_eq!(
            text(&samples, 80),
            "  ink check            # read it\n  ink build -o public  # build into ./public\n"
        );
        assert_eq!(text(&samples, 30), "  # read it\n  ink check\n\n  # build into ./public\n  ink build -o public\n");
    }
}
