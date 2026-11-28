//! A list a command prints: a table under a dim header, or, when there is nothing to list, a
//! line that says so and what to type next.

use super::Table;
use super::flow::Samples;
use crate::output::term::{Doc, Line, Render, Role};

/// What a list says when it has no rows: a sentence, and a command that makes the first one.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Empty {
    /// The sentence: `No previews.`
    pub(crate) said: &'static str,
    /// A line to type that makes a row, and what it does; none when only a stranger can.
    pub(crate) next: Option<(&'static str, &'static str)>,
}

impl Render for Empty {
    fn render(&self, into: &mut Doc) {
        into.push(Line::new().with(Role::Quiet, self.said));
        if let Some((command, about)) = self.next {
            Samples::new(2).row(Line::new().strong(Role::Literal, command), Line::plain(about)).render(into);
        }
    }
}

/// Rows under a dim header, in aligned columns, or the [`Empty`] state.
#[derive(Clone, Debug)]
pub(crate) struct Listed {
    empty: Empty,
    head: Vec<&'static str>,
    rows: Vec<Vec<Line>>,
}

impl Listed {
    /// A list with these column names, which says `empty` when no row is added.
    pub(crate) fn new(empty: Empty, head: &[&'static str]) -> Self {
        Self { empty, head: head.to_vec(), rows: Vec::new() }
    }

    /// One row of cells.
    #[must_use]
    pub(crate) fn row(mut self, cells: Vec<Line>) -> Self {
        self.rows.push(cells);
        self
    }
}

impl Render for Listed {
    fn render(&self, into: &mut Doc) {
        if self.rows.is_empty() {
            self.empty.render(into);
            return;
        }
        let mut table =
            Table::default().indent(2).row(self.head.iter().map(|name| Line::new().with(Role::Quiet, name)).collect());
        for row in &self.rows {
            table = table.row(row.clone());
        }
        table.render(into);
    }
}
