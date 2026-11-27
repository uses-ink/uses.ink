//! The text of the deploy commands' results.
//!
//! These are small typed values that print, so a command builds no output with `format!`. The
//! terminal layer's table and record blocks replace them when it lands; the columns and the words
//! here are the ones `docs/cli.md` section 7 shows.

use std::fmt;

use jiff::Timestamp;
use model::DeployId;

use super::wire::{Built, Live, Moved, Row};

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

impl fmt::Display for Listing<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let size = |row: &Row| Size(row.bytes).to_string();
        let files = self.rows.iter().map(|row| row.files.to_string().len()).max().unwrap_or(0).max("files".len());
        let bytes = self.rows.iter().map(|row| size(row).len()).max().unwrap_or(0).max("size".len());
        writeln!(out, "  {:<SHORT$}  {:<17}  {:>files$}  {:>bytes$}  state", "deploy", "when", "files", "size")?;
        for row in self.rows {
            let state = if row.live { "live" } else { "earlier" };
            writeln!(
                out,
                "  {:<SHORT$}  {:<17}  {:>files$}  {:>bytes$}  {state}",
                Short(row.id).to_string(),
                Moment(row.created).to_string(),
                row.files,
                size(row),
            )?;
        }
        if self.rows.len() < self.total {
            write!(out, "\n{} of {} deploys; --limit shows more", self.rows.len(), self.total)?;
        }
        Ok(())
    }
}

/// One deploy as a record, with its build and the log of it.
#[derive(Debug)]
pub(crate) struct Record<'a> {
    pub(crate) deploy: &'a Row,
    pub(crate) build: Option<&'a Built>,
    pub(crate) log: &'a [String],
}

impl fmt::Display for Record<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let row = self.deploy;
        writeln!(out, "deploy  {}", row.id)?;
        writeln!(out, "when    {}", Moment(row.created))?;
        writeln!(out, "state   {}", if row.live { "live" } else { "earlier" })?;
        write!(out, "files   {}, {}", row.files, Size(row.bytes))?;
        if let Some(build) = self.build {
            write!(out, "\nbuild   {} ({}, {})", build.id, build.state, build.trigger)?;
        }
        if !self.log.is_empty() {
            write!(out, "\nlog")?;
            for line in self.log {
                write!(out, "\n  {line}")?;
            }
        }
        Ok(())
    }
}

/// Where the pointer is after a rollback or a resume.
#[derive(Debug)]
pub(crate) struct Pointer<'a>(pub(crate) &'a Moved);

impl fmt::Display for Pointer<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0.live {
            Some(Live { deploy, paused, .. }) => {
                write!(out, "live    {}{}", deploy, if *paused { " (new deploys are paused)" } else { "" })
            }
            None => write!(out, "live    nothing"),
        }
    }
}
