//! The layout types: the only code that pads or cuts (`docs/cli.md` section 6). Columns align on
//! unstyled text, a glyph counts as the columns it takes, and the one flexible column is cut,
//! with a mark, only when the width is limited.

use unicode_width::UnicodeWidthStr;

use super::doc::{Doc, Line, Render};
use super::glyph::Glyph;
use super::role::{Role, Strength};

/// Where a cut column loses its text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cut {
    /// At its end, as prose is cut.
    End,
    /// In its middle, which keeps both ends of a path.
    Middle,
}

/// The column that absorbs a lack of width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Flex {
    pub(crate) column: usize,
    pub(crate) cut: Cut,
}

/// Rows of cells laid out in aligned columns.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Columns<'a> {
    pub(crate) rows: &'a [Vec<Line>],
    pub(crate) flex: Option<Flex>,
    /// Columns already taken in front of each row.
    pub(crate) lead: usize,
}

impl Columns<'_> {
    /// Columns between two cells.
    const GAP: usize = 2;

    /// Each row as one line: every cell but the last padded to the widest of its column.
    pub(crate) fn lines(&self, doc: &Doc) -> Vec<Line> {
        let count = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut widths: Vec<usize> = (0..count)
            .map(|at| self.rows.iter().filter_map(|row| row.get(at)).map(Line::width).max().unwrap_or(0))
            .collect();
        let used = self.lead + widths.iter().sum::<usize>() + Self::GAP * count.saturating_sub(1);
        if let Some(flex) = self.flex
            && used > doc.room()
            && let Some(width) = widths.get_mut(flex.column)
        {
            let floor = doc.look().glyphs.get(Glyph::Truncation).width() + 1;
            *width = doc.room().saturating_sub(used - *width).max(floor).min(*width);
        }
        self.rows.iter().map(|row| self.row(row, &widths, doc)).collect()
    }

    fn row(&self, row: &[Line], widths: &[usize], doc: &Doc) -> Line {
        let Some(last) = row.iter().rposition(|cell| !cell.is_empty()) else { return Line::new() };
        let mut line = Line::new();
        for (at, cell) in row.iter().enumerate() {
            let width = widths.get(at).copied().unwrap_or(0);
            let cell = match self.flex {
                Some(flex) if flex.column == at => cell.cut(width, flex.cut, doc.look().glyphs),
                _ => cell.clone(),
            };
            let fill = width.saturating_sub(cell.width());
            line = line.then(cell);
            if at == last {
                break;
            }
            line = line.pad(fill + Self::GAP);
        }
        line
    }
}

/// A subject, then rows joined to it by branches, columns inside the rows.
#[derive(Clone, Debug, Default)]
pub(crate) struct Tree {
    subject: Line,
    rows: Vec<Vec<Line>>,
    flex: Option<Flex>,
    indent: usize,
}

impl Tree {
    /// A tree under `subject`.
    pub(crate) fn new(subject: Line) -> Self {
        Self { subject, ..Self::default() }
    }

    /// A row of cells.
    #[must_use]
    pub(crate) fn row(mut self, cells: Vec<Line>) -> Self {
        self.rows.push(cells);
        self
    }

    /// The column that is cut when the width falls short.
    #[must_use]
    pub(crate) fn flex(mut self, column: usize, cut: Cut) -> Self {
        self.flex = Some(Flex { column, cut });
        self
    }

    /// Everything in the tree is indented by `columns`.
    #[must_use]
    pub(crate) fn indent(mut self, columns: usize) -> Self {
        self.indent = columns;
        self
    }

    /// Whether the tree has no rows.
    pub(crate) fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl Render for Tree {
    fn render(&self, into: &mut Doc) {
        if !self.subject.is_empty() {
            into.push(Line::new().pad(self.indent).then(self.subject.clone()));
        }
        let glyphs = into.look().glyphs;
        let lead = self.indent + glyphs.get(Glyph::Branch).width() + 1;
        let lines = Columns { rows: &self.rows, flex: self.flex, lead }.lines(into);
        let count = lines.len();
        for (at, line) in lines.into_iter().enumerate() {
            let branch = into.glyph(if at + 1 == count { Glyph::Last } else { Glyph::Branch });
            into.push(Line::new().pad(self.indent).then(branch).pad(1).then(line));
        }
    }
}

/// Several trees, a blank line between each.
#[derive(Clone, Debug, Default)]
pub(crate) struct Trees(pub(crate) Vec<Tree>);

impl Render for Trees {
    fn render(&self, into: &mut Doc) {
        for (at, tree) in self.0.iter().enumerate() {
            if at > 0 {
                into.blank();
            }
            tree.render(into);
        }
    }
}

/// A heading, then rows in aligned columns.
#[derive(Clone, Debug, Default)]
pub(crate) struct Table {
    title: Option<Line>,
    rows: Vec<Vec<Line>>,
    flex: Option<Flex>,
    indent: usize,
}

impl Table {
    /// A table under `title`.
    pub(crate) fn new(title: Line) -> Self {
        Self { title: Some(title), ..Self::default() }
    }

    /// A row of cells.
    #[must_use]
    pub(crate) fn row(mut self, cells: Vec<Line>) -> Self {
        self.rows.push(cells);
        self
    }

    /// The column that is cut when the width falls short.
    #[must_use]
    pub(crate) fn flex(mut self, column: usize, cut: Cut) -> Self {
        self.flex = Some(Flex { column, cut });
        self
    }

    /// The rows are indented by `columns`.
    #[must_use]
    pub(crate) fn indent(mut self, columns: usize) -> Self {
        self.indent = columns;
        self
    }
}

impl Render for Table {
    fn render(&self, into: &mut Doc) {
        if let Some(title) = &self.title {
            into.push(title.clone());
        }
        for line in (Columns { rows: &self.rows, flex: self.flex, lead: self.indent }).lines(into) {
            into.push(Line::new().pad(self.indent).then(line));
        }
    }
}

/// Keys, dim, aligned, then values.
#[derive(Clone, Debug, Default)]
pub(crate) struct Record {
    rows: Vec<Vec<Line>>,
    indent: usize,
}

impl Record {
    /// A record whose rows are indented by `columns`.
    pub(crate) fn new(indent: usize) -> Self {
        Self { rows: Vec::new(), indent }
    }

    /// One key and its value.
    #[must_use]
    pub(crate) fn field(mut self, key: &str, value: Line) -> Self {
        self.rows.push(vec![Line::new().with(Role::Quiet, key), value]);
        self
    }
}

impl Render for Record {
    fn render(&self, into: &mut Doc) {
        for line in (Columns { rows: &self.rows, flex: None, lead: self.indent }).lines(into) {
            into.push(Line::new().pad(self.indent).then(line));
        }
    }
}

/// One bare line: the result of a command, as a script reads it.
#[derive(Clone, Debug)]
pub(crate) struct Answer(pub(crate) Line);

impl Answer {
    /// `text`, as it is.
    pub(crate) fn of(text: impl std::fmt::Display) -> Self {
        Self(Line::plain(text.to_string()))
    }
}

impl Render for Answer {
    fn render(&self, into: &mut Doc) {
        into.push(self.0.clone());
    }
}

/// A line that starts with a past-tense verb, right-aligned in a gutter, as cargo's do.
#[derive(Clone, Debug)]
pub(crate) struct Gutter {
    pub(crate) word: String,
    pub(crate) role: Role,
    pub(crate) strength: Strength,
    pub(crate) text: Line,
}

impl Gutter {
    /// Columns the verbs are right-aligned in.
    pub(crate) const WIDTH: usize = 10;
    /// Columns in front of the gutter.
    pub(crate) const MARGIN: usize = 2;
}

impl Render for Gutter {
    fn render(&self, into: &mut Doc) {
        let fill = Self::WIDTH.saturating_sub(self.word.width());
        let line = Line::new().pad(Self::MARGIN + fill);
        let line = match self.strength {
            Strength::Strong => line.strong(self.role, &self.word),
            Strength::Normal => line.with(self.role, &self.word),
        };
        into.push(line.pad(2).then(self.text.clone()));
    }
}

/// A rail with a head and a foot and aligned rows between.
#[derive(Clone, Debug)]
pub(crate) struct Rail {
    pub(crate) top: Line,
    pub(crate) rows: Vec<Vec<Line>>,
    pub(crate) foot: Line,
    pub(crate) flex: Option<Flex>,
}

impl Render for Rail {
    fn render(&self, into: &mut Doc) {
        let glyphs = into.look().glyphs;
        let rail = || into.glyph(Glyph::Rail);
        let first = into.glyph(Glyph::Top).pad(1).then(self.top.clone());
        let lead = glyphs.get(Glyph::Rail).width() + 1;
        let lines = Columns { rows: &self.rows, flex: self.flex, lead }.lines(into);
        let rows: Vec<Line> = lines.into_iter().map(|line| rail().pad(1).then(line)).collect();
        let (blank, last) = (rail(), into.glyph(Glyph::Last).pad(1).then(self.foot.clone()));
        into.push(first);
        into.push(blank);
        for row in rows {
            into.push(row);
        }
        into.push(last);
    }
}

/// Text that already has its lines, written as it is: a script, a manual page, a listing.
#[derive(Clone, Debug)]
pub(crate) struct Verbatim(pub(crate) String);

impl Render for Verbatim {
    fn render(&self, into: &mut Doc) {
        for line in self.0.lines() {
            into.push(Line::plain(line));
        }
    }
}

/// Text that was painted by the terminal layer's own [`Paint`](crate::help::Paint), such as a
/// page of help: its escape codes are kept when colour shows and stripped when it does not.
#[derive(Clone, Debug)]
pub(crate) struct Styled(pub(crate) String);

impl Render for Styled {
    fn render(&self, into: &mut Doc) {
        for line in self.0.lines() {
            let mut whole = Line::new();
            whole.push(super::doc::Piece::ansi(line.to_owned()));
            into.push(whole);
        }
    }
}
