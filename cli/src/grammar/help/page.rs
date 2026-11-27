//! A help page under construction: headings, paragraphs and two aligned columns.

use super::paint::{Face, Paint};

/// One row of two columns.
#[derive(Clone, Debug)]
pub(crate) struct Row {
    /// What you type, or the key of the row.
    pub(crate) left: String,
    /// What it is, wrapped under itself.
    pub(crate) right: String,
    /// Whether the left column is something you type, and so is painted as such.
    pub(crate) typed: bool,
}

impl Row {
    /// A row whose left column is something you type.
    pub(crate) fn typed(left: impl Into<String>, right: impl Into<String>) -> Self {
        Self { left: left.into(), right: right.into(), typed: true }
    }

    /// A row whose left column is a key, such as an exit code.
    pub(crate) fn keyed(left: impl Into<String>, right: impl Into<String>) -> Self {
        Self { left: left.into(), right: right.into(), typed: false }
    }
}

/// The text of a page, written as it is built.
#[derive(Debug)]
pub(crate) struct Page<'p, P> {
    paint: &'p P,
    width: usize,
    text: String,
}

impl<'p, P: Paint> Page<'p, P> {
    /// How far a row is indented.
    const INDENT: usize = 2;

    /// An empty page that wraps at `width` columns.
    pub(crate) const fn new(paint: &'p P, width: usize) -> Self {
        Self { paint, width, text: String::new() }
    }

    /// The text so far.
    pub(crate) fn finish(self) -> String {
        self.text
    }

    /// A blank line, unless the page is empty or already ends in one.
    pub(crate) fn gap(&mut self) {
        if !self.text.is_empty() && !self.text.ends_with("\n\n") {
            self.text.push('\n');
        }
    }

    /// A heading line.
    pub(crate) fn heading(&mut self, heading: &str) {
        self.gap();
        let painted = self.paint.paint(Face::Heading, heading);
        self.text.push_str(&painted);
        self.text.push('\n');
    }

    /// A paragraph, wrapped.
    pub(crate) fn paragraph(&mut self, text: &str) {
        self.gap();
        for line in Wrap::new(self.width).lines(text) {
            self.text.push_str(&line);
            self.text.push('\n');
        }
    }

    /// A line as it is, such as a usage line or a pointer to another page.
    pub(crate) fn line(&mut self, text: &str) {
        self.gap();
        self.text.push_str(text);
        self.text.push('\n');
    }

    /// `heading`, then `rows` in two columns: the first as wide as its longest entry plus two,
    /// measured on unstyled text, the second wrapped with a hanging indent.
    pub(crate) fn table(&mut self, heading: &str, rows: &[Row]) {
        self.heading(heading);
        self.rows(rows);
    }

    /// `rows` in two columns, under whatever came before.
    pub(crate) fn rows(&mut self, rows: &[Row]) {
        let column = rows.iter().map(|row| row.left.chars().count()).max().unwrap_or(0) + 2;
        let room = self.width.saturating_sub(Self::INDENT + column).max(20);
        for row in rows {
            let left = if row.typed { self.paint.paint(Face::Literal, &row.left) } else { row.left.clone() };
            let pad = column.saturating_sub(row.left.chars().count());
            let mut lines = Wrap::new(room).lines(&row.right).into_iter();
            self.text.push_str(&" ".repeat(Self::INDENT));
            self.text.push_str(&left);
            if let Some(first) = lines.next() {
                self.text.push_str(&" ".repeat(pad));
                self.text.push_str(&first);
            }
            self.text.push('\n');
            for rest in lines {
                self.text.push_str(&" ".repeat(Self::INDENT + column));
                self.text.push_str(&rest);
                self.text.push('\n');
            }
        }
    }
}

/// Greedy word wrap at a width in characters.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Wrap {
    width: usize,
}

impl Wrap {
    /// Wraps at `width`.
    pub(crate) const fn new(width: usize) -> Self {
        Self { width }
    }

    /// `text` as lines no longer than the width, except for a word that is longer itself.
    pub(crate) fn lines(self, text: &str) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        for paragraph in text.split('\n') {
            let mut line = String::new();
            for word in paragraph.split_whitespace() {
                let fits = line.is_empty() || line.chars().count() + 1 + word.chars().count() <= self.width;
                if !fits {
                    lines.push(std::mem::take(&mut line));
                }
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
            }
            lines.push(line);
        }
        lines
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::term::Look;

    #[test]
    fn the_second_column_starts_two_past_the_longest_first() {
        let mut page = Page::new(&Look::PLAIN, 80);
        page.rows(&[Row::typed("a", "one"), Row::typed("long-name", "two")]);
        assert_eq!(page.finish(), "  a          one\n  long-name  two\n");
    }

    #[test]
    fn a_long_description_hangs_under_itself() {
        let mut page = Page::new(&Look::PLAIN, 30);
        page.rows(&[Row::keyed("x", "aaaa bbbb cccc dddd eeee ffff gggg hhhh iiii jjjj kkkk llll mmmm nnnn")]);
        let text = page.finish();
        assert!(text.lines().count() > 1, "{text}");
        assert!(text.lines().skip(1).all(|line| line.starts_with("     ")), "{text}");
    }

    #[test]
    fn styled_text_does_not_skew_the_columns() {
        let mut page = Page::new(&Look { colour: true, ..Look::PLAIN }, 80);
        page.rows(&[Row::typed("ab", "x"), Row::typed("abcd", "y")]);
        let text = page.finish();
        let plain: String = {
            let mut out = String::new();
            let mut skip = false;
            for c in text.chars() {
                match (skip, c) {
                    (_, '\u{1b}') => skip = true,
                    (true, 'm') => skip = false,
                    (false, c) => out.push(c),
                    _ => {}
                }
            }
            out
        };
        assert_eq!(plain, "  ab    x\n  abcd  y\n");
    }
}
