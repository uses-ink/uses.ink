//! The start report for a person: a header, the three addresses to open, the first-run link in a
//! block of its own, then a table of settings for each part.

use crate::output::term::{Cut, Doc, Glyph, Line, Rail, Render, Role, Table};

use super::{Admin, Caution, Row, Section, Stage, Startup, Urls};

impl Startup {
    /// What a person sees first: the program, where it listens and how far it got.
    fn header(&self) -> Line {
        let verb = match self.stage {
            Stage::Serving => "listening on",
            Stage::Planned => "would listen on",
        };
        let address = self.address.trim_start_matches("http://");
        let line = Line::new()
            .strong(Role::Heading, self.release.name)
            .pad(1)
            .with(Role::Quiet, self.release.version)
            .pad(2)
            .strong(Role::Heading, "serve")
            .pad(2)
            .with(Role::Quiet, verb)
            .pad(1)
            .with(Role::Figure, address);
        match self.stage {
            Stage::Serving => line,
            Stage::Planned => line.with(Role::Quiet, " (--print-config: nothing was bound, made or written)"),
        }
    }

    /// The addresses to open, the emphasised block.
    fn addresses(&self, into: &mut Doc) {
        let Urls { site, dashboard, api } = &self.urls;
        let row = |name: &str, url: &str| {
            Line::new().pad(2).with(Role::Quiet, format!("{name:<9}")).pad(2).strong(Role::Link, url)
        };
        into.push(row("site", site));
        if let (Some(dashboard), Some(api)) = (dashboard, api) {
            into.push(row("dashboard", dashboard));
            into.push(row("api", api));
        }
    }

    /// The first-run link: its own block, whole on one line, with how long it works.
    fn admin(&self, into: &mut Doc) {
        match &self.admin {
            Admin::Issued { link, valid } => {
                let top = Line::new()
                    .strong(Role::Heading, "admin link")
                    .pad(2)
                    .with(Role::Quiet, "works for ")
                    .then(valid.line())
                    .with(Role::Quiet, ", shown once and kept nowhere");
                let rail = Rail {
                    top,
                    rows: vec![vec![Line::new().strong(Role::Link, link)]],
                    foot: Line::new().with(Role::Quiet, "opening it signs you in as the owner"),
                    flex: None,
                };
                rail.render(into);
            }
            Admin::Kept => into.push(
                Line::new()
                    .strong(Role::Heading, "admin link")
                    .pad(2)
                    .with(Role::Quiet, "not made (one was made before, or an account can sign in)"),
            ),
            Admin::Unminted => into.push(
                Line::new()
                    .strong(Role::Heading, "admin link")
                    .pad(2)
                    .with(Role::Quiet, "not made by --print-config; a real start makes it once"),
            ),
            Admin::Off => {}
        }
    }
}

impl Section {
    /// The part as a titled table of key, value and source, and the cautions below it.
    fn table(&self) -> Table {
        let mut table = Table::new(Line::new().strong(Role::Heading, self.part.title())).indent(2).flex(2, Cut::End);
        for row in &self.rows {
            table = table.row(vec![Line::new().with(Role::Quiet, row.label()), row.value.line(), row.source.line()]);
        }
        table
    }

    /// The cautions of the rows, each with its fix.
    fn cautions(&self) -> impl Iterator<Item = (&Row, &Caution)> {
        self.rows.iter().filter_map(|row| row.caution.as_ref().map(|caution| (row, caution)))
    }
}

impl Startup {
    /// `text` after `lead`, wrapped to the line with later lines indented by `rest` columns.
    fn hang(into: &mut Doc, lead: Line, rest: usize, text: Line) {
        let room = into.room().saturating_sub(rest);
        if lead.width() + text.width() <= into.room() {
            into.push(lead.then(text));
            return;
        }
        for (at, line) in text.wrap(room, room).into_iter().enumerate() {
            let start = if at == 0 { lead.clone() } else { Line::new().pad(rest) };
            into.push(start.then(line));
        }
    }
}

impl Render for Startup {
    fn render(&self, into: &mut Doc) {
        into.push(self.header());
        into.blank();
        self.addresses(into);
        if !matches!(self.admin, Admin::Off) {
            into.blank();
            self.admin(into);
        }
        for section in &self.sections {
            into.blank();
            section.table().render(into);
            for (row, caution) in section.cautions() {
                let mark = into.glyph(Glyph::Changed);
                let said = Line::new().with(Role::Warn, format!("{}: {}", row.label(), caution.message()));
                Self::hang(into, Line::new().pad(2).then(mark).pad(1), 4, said);
                let fix = Line::new().with(Role::Literal, "fix:").pad(1).then(Line::marked(Role::Warn, &caution.fix()));
                Self::hang(into, Line::new().pad(4), 9, fix);
            }
        }
        into.blank();
        let last = match self.stage {
            Stage::Serving => {
                Line::new().with(Role::Quiet, "press ").with(Role::Literal, "ctrl-c").with(Role::Quiet, " to stop")
            }
            Stage::Planned => Line::new().with(Role::Quiet, "nothing is running; start it without --print-config"),
        };
        into.push(last);
    }
}
