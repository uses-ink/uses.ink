//! The parts a page of help is made of, pushed as lines of roles into one document: sections with
//! a heading, paragraphs, and the lines of an example with their commands marked.

use crate::grammar::tree::{Located, Tree};
use crate::output::term::{Doc, Line, Look, Paragraph, Render, Role, Width};

/// A page under construction: blocks, each after a blank line.
#[derive(Debug)]
pub(crate) struct Sheet {
    doc: Doc,
}

impl Sheet {
    /// An empty page that looks like `look`.
    pub(crate) fn new(look: Look) -> Self {
        Self { doc: Doc::new(look) }
    }

    /// The finished document.
    pub(crate) fn finish(self) -> Doc {
        self.doc
    }

    /// A blank line, unless the page is empty or already ends in one.
    fn gap(&mut self) {
        if self.doc.lines().last().is_some_and(|line| !line.is_empty()) {
            self.doc.blank();
        }
    }

    /// A line as it is, after a blank one.
    pub(crate) fn line(&mut self, line: Line) {
        self.gap();
        self.doc.push(line);
    }

    /// A paragraph, wrapped.
    pub(crate) fn paragraph(&mut self, line: Line) {
        self.gap();
        Paragraph::new(line, 0).render(&mut self.doc);
    }

    /// `block` as it is, after a blank line.
    pub(crate) fn block(&mut self, block: &impl Render) {
        self.gap();
        block.render(&mut self.doc);
    }

    /// A heading, then `block` under it.
    pub(crate) fn section(&mut self, heading: &str, block: &impl Render) {
        self.gap();
        self.doc.push(Line::new().with(Role::Heading, heading));
        block.render(&mut self.doc);
    }
}

/// How wide a page is: the width the stream gives, between 40 and 100 columns, and 80 when the
/// stream has no limit (a pipe, or `INK_WIDTH=0`).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Measure;

impl Measure {
    /// The columns of the narrowest page.
    const LEAST: usize = 40;
    /// The columns of the widest page.
    const MOST: usize = 100;
    /// The columns of a page for a stream with no limit.
    const USUAL: usize = 80;

    /// `look`, with the width a page of help is wrapped at.
    pub(crate) const fn of(look: Look) -> Look {
        let columns = match look.width {
            Width::Unlimited => Self::USUAL,
            Width::Columns(columns) if columns < Self::LEAST => Self::LEAST,
            Width::Columns(columns) if columns > Self::MOST => Self::MOST,
            Width::Columns(columns) => columns,
        };
        Look { width: Width::Columns(columns), ..look }
    }
}

/// A line to type, its words in the roles they have: commands of the tree strong, flags literal,
/// what is given to them plain.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Typed<'a> {
    pub(crate) tree: &'a Tree,
    pub(crate) text: &'a str,
}

impl Typed<'_> {
    /// The line, a word at a time.
    pub(crate) fn line(self) -> Line {
        let mut line = Line::new();
        let mut here = Located::root(self.tree);
        for (at, word) in self.text.split(' ').enumerate() {
            if at > 0 {
                line = line.with(Role::Plain, " ");
            }
            line = if at == 0 {
                line.strong(Role::Literal, word)
            } else if word.starts_with('-') {
                line.with(Role::Literal, word)
            } else if let Some(next) = here.enter(word) {
                here = next;
                line.strong(Role::Literal, word)
            } else {
                line.with(Role::Plain, word)
            };
        }
        line
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_width_is_clamped_and_a_pipe_gets_eighty() {
        let width = |width| Measure::of(Look { width, ..Look::PLAIN }).width;
        assert_eq!(width(Width::Unlimited), Width::Columns(80));
        assert_eq!(width(Width::Columns(10)), Width::Columns(40));
        assert_eq!(width(Width::Columns(60)), Width::Columns(60));
        assert_eq!(width(Width::Columns(300)), Width::Columns(100));
    }

    #[test]
    fn the_commands_of_a_line_are_marked_and_its_arguments_are_not() {
        let line = Typed { tree: Tree::ink(), text: "ink -C garden build -o public" }.line();
        let roles: Vec<(Role, String)> = line
            .pieces()
            .iter()
            .filter(|piece| piece.text != " ")
            .map(|piece| (piece.role, piece.text.clone()))
            .collect();
        assert_eq!(
            roles,
            [
                (Role::Literal, "ink".to_owned()),
                (Role::Literal, "-C".to_owned()),
                (Role::Plain, "garden".to_owned()),
                (Role::Literal, "build".to_owned()),
                (Role::Literal, "-o".to_owned()),
                (Role::Plain, "public".to_owned()),
            ]
        );
    }
}
