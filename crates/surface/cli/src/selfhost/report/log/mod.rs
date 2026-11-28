//! The operator log of `ink serve`: one line for each thing the server, its loops and its mail
//! sender did, as typed [`Entry`] values written one way for a person and one way for a machine
//! (decision 0210).
//!
//! What happens is a [`Happens`]: a level, a component, a stable name, a message and typed facts.
//! The [`Logger`] decides what is shown (the level, and whether ordinary request lines are) and
//! how: through the terminal layer, with its roles, glyphs, width and colour decisions, or as one
//! JSON object a line with no colour.
//!
//! This is the log of the server's work for its operator. The tracing of decision 0065, for
//! debugging the program itself, is a separate channel (`--log`, `-vvv`) and stays that way.

mod entry;
mod happens;
#[cfg(test)]
mod test;

use std::io::Write as _;
use std::sync::{Arc, Mutex};

use clap::ValueEnum;

use crate::grammar::args::Serve;
use crate::output::term::{Look, Writer};
use crate::output::{Io, Say};

pub(crate) use entry::{Component, Datum, Entry, Fact, Level, Remedy, Stamp, Zone};
pub(crate) use happens::Happens;

/// How the log is written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Shape {
    /// Lines for a person: a dim time, a coloured level, the component, the message and its facts.
    #[default]
    Human,
    /// One JSON object a line, with stable field names and no colour.
    Json,
}

/// What a logger shows and how.
#[derive(Debug)]
struct Settings {
    shape: Shape,
    floor: Level,
    requests: bool,
    zone: Zone,
}

/// Writes the entries of a serve to stderr. Clones share one stream, so the server, the loops and
/// the mail sender write lines that do not mix.
#[derive(Clone, Debug)]
pub(crate) struct Logger {
    settings: Arc<Settings>,
    sink: Arc<Mutex<Writer<std::io::Stderr>>>,
}

impl Logger {
    /// A logger for `serve` on the streams of `io`. `--json` makes the log JSON, since stderr is a
    /// machine channel then; `-q` and `-qq` lower the level, and `-v` turns on request lines.
    pub(crate) fn new(io: &Io<'_>, serve: &Serve) -> Self {
        let (look, machine) = io.logged();
        let shape = if machine { Shape::Json } else { serve.log_format };
        let floor = match (io.says(Say::Warnings), io.says(Say::Stages)) {
            (false, _) => Level::Error,
            (true, false) => serve.log_level.min(Level::Warn),
            (true, true) => serve.log_level,
        };
        let look = Look { colour: look.colour && shape == Shape::Human, ..look };
        Self {
            settings: Arc::new(Settings { shape, floor, requests: io.says(Say::Notices), zone: serve.log_time }),
            sink: Arc::new(Mutex::new(Writer::new(std::io::stderr(), look))),
        }
    }

    /// Whether `event` is shown: at or above the level, and an ordinary request only when asked.
    fn shows(&self, event: &dyn Happens) -> bool {
        event.level() <= self.settings.floor && (!event.request() || self.settings.requests)
    }

    /// Writes one line for `event`, when it is shown.
    pub(crate) fn tell(&self, event: &dyn Happens) {
        if !self.shows(event) {
            return;
        }
        let entry = Entry::of(event, Stamp { at: jiff::Timestamp::now(), zone: self.settings.zone });
        let mut sink = self.sink.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        // a closed stderr leaves nowhere to report to
        match self.settings.shape {
            Shape::Human => {
                sink.emit(&entry).ok();
            }
            Shape::Json => {
                if serde_json::to_writer(sink.sink(), &entry).is_ok() {
                    sink.sink().write_all(b"\n").ok();
                }
            }
        }
    }
}

impl edge_axum::Observe for Logger {
    fn event(&self, event: &edge_axum::Event) {
        self.tell(event);
    }
}

impl edge_api::Notes for Logger {
    fn note(&self, note: &edge_api::Note) {
        self.tell(note);
    }
}
