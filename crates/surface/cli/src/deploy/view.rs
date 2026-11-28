//! The deploy commands' results as typed blocks of the terminal layer.
//!
//! Each is a value that renders into a table or a record, so a command builds no output with
//! `format!`; the columns and the words are the ones `docs/cli.md` section 7 shows.

use std::fmt;

use jiff::Timestamp;
use model::DeployId;
use serde::Serialize;

use crate::output::term::{Doc, Line, Record as Fields, Render, Role, Table};
use edge_api::view::{BuildView as Built, DeployView as Row, LiveView as Live, Moved};

/// How many characters of a deploy id are shown.
pub(crate) const SHORT: usize = 12;

/// A deploy id, shortened to what help and listings show.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Short(pub(crate) DeployId);

impl fmt::Display for Short {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{:.SHORT$}", self.0.to_string())
    }
}

/// A moment, UTC and ISO, as tables show it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Moment(pub(crate) Timestamp);

impl fmt::Display for Moment {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0.strftime("%Y-%m-%d %H:%MZ"))
    }
}

/// A size in bytes, as a person reads it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Size(pub(crate) u64);

impl fmt::Display for Size {
    #[expect(clippy::cast_precision_loss, reason = "a size shown to one decimal place")]
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        const UNITS: [&str; 4] = ["B", "kB", "MB", "GB"];
        let mut value = self.0 as f64;
        let mut unit = 0;
        while value >= 1000.0 && unit + 1 < UNITS.len() {
            value /= 1000.0;
            unit += 1;
        }
        match UNITS.get(unit) {
            Some(name) if unit == 0 => write!(out, "{} {name}", self.0),
            Some(name) => write!(out, "{value:.1} {name}"),
            None => write!(out, "{} B", self.0),
        }
    }
}

/// The history: one line per deploy, then how many of how many.
#[derive(Debug)]
pub(crate) struct Listing<'a> {
    pub(crate) rows: &'a [Row],
    pub(crate) total: usize,
}

impl Render for Listing<'_> {
    fn render(&self, into: &mut Doc) {
        let quiet = |text: &str| Line::new().with(Role::Quiet, text);
        let mut table =
            Table::default().indent(2).row(["deploy", "when", "files", "size", "state"].map(quiet).to_vec());
        for row in self.rows {
            let (role, state) = if row.live { (Role::Good, "live") } else { (Role::Quiet, "earlier") };
            table = table.row(vec![
                Line::new().with(Role::Id, Short(row.id).to_string()),
                Line::plain(Moment(row.created).to_string()),
                Line::plain(row.files.to_string()),
                Line::plain(Size(row.bytes.count()).to_string()),
                Line::new().with(role, state),
            ]);
        }
        table.render(into);
        if self.rows.len() < self.total {
            into.blank();
            into.push(quiet(&format!("{} of {} deploys; --limit shows more", self.rows.len(), self.total)));
        }
    }
}

/// One deploy as a record, with its build and the log of it.
#[derive(Debug)]
pub(crate) struct Record<'a> {
    pub(crate) deploy: &'a Row,
    pub(crate) build: Option<&'a Built>,
    pub(crate) log: &'a [String],
}

impl Render for Record<'_> {
    fn render(&self, into: &mut Doc) {
        let row = self.deploy;
        let (role, state) = if row.live { (Role::Good, "live") } else { (Role::Quiet, "earlier") };
        let mut fields = Fields::new(0)
            .field("deploy", Line::new().with(Role::Id, row.id.to_string()))
            .field("when", Line::plain(Moment(row.created).to_string()))
            .field("state", Line::new().with(role, state))
            .field("files", Line::plain(format!("{}, {}", row.files, Size(row.bytes.count()))));
        if let Some(build) = self.build {
            fields = fields.field("build", Line::plain(format!("{} ({}, {})", build.id, build.state, build.trigger)));
        }
        fields.render(into);
        if !self.log.is_empty() {
            into.push(Line::new().with(Role::Quiet, "log"));
            for line in self.log {
                into.push(Line::new().pad(2).then(Line::plain(line)));
            }
        }
    }
}

/// Where the pointer is after a rollback or a resume.
#[derive(Debug)]
pub(crate) struct Pointer<'a>(pub(crate) &'a Moved);

impl Render for Pointer<'_> {
    fn render(&self, into: &mut Doc) {
        let live = match &self.0.live {
            Some(Live { deploy, paused, .. }) => {
                let line = Line::new().with(Role::Id, deploy.to_string());
                if *paused { line.with(Role::Warn, " (new deploys are paused)") } else { line }
            }
            None => Line::plain("nothing"),
        };
        Fields::new(0).field("live", live).render(into);
    }
}

/// What `ink deploy list --json` prints.
#[derive(Debug, Serialize)]
pub(crate) struct TableOut<'a> {
    pub(crate) schema: &'static str,
    pub(crate) site: String,
    pub(crate) total: usize,
    pub(crate) deploys: &'a [Row],
}

/// What `ink deploy show --json` prints.
#[derive(Debug, Serialize)]
pub(crate) struct ShowOut<'a> {
    pub(crate) schema: &'static str,
    pub(crate) deploy: &'a Row,
    pub(crate) build: Option<&'a Built>,
    /// The lines of the build's log, each one NDJSON as the API sent it.
    pub(crate) log: &'a [String],
}

/// What `ink deploy rollback` and `ink deploy resume --json` print.
#[derive(Debug, Serialize)]
pub(crate) struct MoveOut<'a> {
    pub(crate) schema: &'static str,
    pub(crate) site: String,
    pub(crate) moved: &'a Moved,
}
