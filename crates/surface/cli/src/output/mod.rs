//! Where a command writes: its result to stdout, everything else to stderr, as text through the
//! renderer or, with `--json`, as the one document on stdout and NDJSON events on stderr.

pub(crate) mod error;
pub(crate) mod exit;
pub(crate) mod failure;
#[cfg(test)]
mod golden;
pub(crate) mod report;
pub(crate) mod stage;
pub(crate) mod term;

use std::fmt;
use std::io::Write;
use std::path::Path;

use clock::System;
use model::{Diagnostic, Notice, Severity};
use serde::Serialize;

use crate::output::error::{Error, SourceError};
use crate::output::exit::Exit;
use crate::output::failure::Failure;
use crate::output::report::{Event, Noted, Shown, Texts};
use crate::output::stage::StageLine;
use crate::output::term::{
    Answer, Draw, Elapsed, Line, Look, Meter, Pacing, Phase, Render, Show, Styled, Terminal, Verbatim, Writer,
};

/// How much a command says, from `-qq` to `-vv`. Each level adds to the one before.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Say {
    /// Errors and the exit code.
    Errors,
    /// Warnings, the result, and the newly public list of a change set.
    Warnings,
    /// Stage lines, the summary, the whole change set and live progress.
    Stages,
    /// Notices.
    Notices,
    /// One line for each file read and page rendered, and timings.
    Files,
}

impl Say {
    /// The level `-q` and `-v` counts give; `-q` wins.
    pub(crate) const fn of(quiet: u8, verbose: u8) -> Self {
        match (quiet, verbose) {
            (2.., _) => Self::Errors,
            (1, _) => Self::Warnings,
            (0, 0) => Self::Stages,
            (0, 1) => Self::Notices,
            (0, 2..) => Self::Files,
        }
    }
}

/// How a run reports: as JSON or text, how much, and how each stream looks.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Mode {
    pub(crate) json: bool,
    pub(crate) say: Say,
    pub(crate) terminal: Terminal,
}

/// A command's two streams and how it speaks on them.
pub(crate) struct Io<'a> {
    out: Writer<&'a mut dyn Write>,
    err: Writer<&'a mut dyn Write>,
    json: bool,
    say: Say,
    meter: Meter<System>,
    /// The last thing on stderr was a stage line, so a diagnostic starts after a blank line.
    gap: bool,
}

impl fmt::Debug for Io<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("Io").field("json", &self.json).finish_non_exhaustive()
    }
}

/// What a command said: how many errors and warnings, for its exit code.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Tally {
    pub(crate) errors: usize,
    pub(crate) warnings: usize,
}

impl Tally {
    /// The exit code of a command that ran to its end.
    pub(crate) fn exit(self, strict: Strict) -> Exit {
        if self.errors > 0 {
            Exit::Content
        } else if self.warnings > 0 && strict == Strict::On {
            Exit::Strict
        } else {
            Exit::Done
        }
    }
}

/// Whether warnings fail the command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Strict {
    On,
    Off,
}

impl From<bool> for Strict {
    fn from(on: bool) -> Self {
        if on { Self::On } else { Self::Off }
    }
}

impl<'a> Io<'a> {
    pub(crate) fn new(out: &'a mut dyn Write, err: &'a mut dyn Write, mode: Mode) -> Self {
        let show = match (mode.say, mode.json, mode.terminal.live) {
            (say, ..) if say < Say::Stages => Show::Off,
            (_, true, _) => Show::Events,
            (_, false, true) => Show::Live,
            (_, false, false) => Show::Log,
        };
        Self {
            out: Writer::new(out, mode.terminal.out),
            err: Writer::new(err, mode.terminal.err),
            json: mode.json,
            say: mode.say,
            meter: Meter::new(System, Pacing::default(), show),
            gap: false,
        }
    }

    /// Whether the run says at least `at`.
    pub(crate) fn says(&self, at: Say) -> bool {
        self.say >= at
    }

    /// The result as text on stdout, or nothing with `--json` (see [`Io::json`]).
    pub(crate) fn result(&mut self, block: &impl Render) -> Result<(), Error> {
        if !self.json {
            self.out.emit(block)?;
        }
        Ok(())
    }

    /// The result as lines of text on stdout, or nothing with `--json`: for a command whose result
    /// is a script, a page or a listing that its own `Display` writes.
    pub(crate) fn line(&mut self, text: impl fmt::Display) -> Result<(), Error> {
        self.result(&Verbatim(text.to_string()))
    }

    /// How the result is written for a person: stdout's look, except that a run that writes JSON
    /// has no colour in the text it carries.
    pub(crate) const fn prose(&self) -> Look {
        Look { colour: self.out.look().colour && !self.json, ..self.out.look() }
    }

    /// The result as text that was painted for this stream's look (see [`Styled`]), or nothing
    /// with `--json`.
    pub(crate) fn painted(&mut self, text: &str) -> Result<(), Error> {
        self.result(&Styled(text.to_owned()))
    }

    /// A block beside the result, such as a build's change set, as text on stderr when the run
    /// says at least `at`, or nothing with `--json`, whose result carries it.
    pub(crate) fn aside(&mut self, at: Say, block: &impl Render) -> Result<(), Error> {
        if !self.json && self.say >= at {
            self.err.emit(block)?;
            self.gap = false;
        }
        Ok(())
    }

    /// A stage line on stderr, or with `--json` the event of a stage that finished.
    pub(crate) fn stage(&mut self, line: &StageLine) -> Result<(), Error> {
        if self.say < Say::Stages {
            return Ok(());
        }
        if self.json {
            if let Some(beat) = line.beat() {
                self.event(&beat)?;
            }
        } else {
            self.err.emit(line)?;
            self.gap = true;
        }
        Ok(())
    }

    /// A stage of `total` units begins.
    pub(crate) fn begin(&mut self, phase: Phase, total: u64, unit: &'static str) {
        self.meter.begin(phase, total, unit);
    }

    /// `done` units are finished, `item` the latest started.
    pub(crate) fn tick(&mut self, done: u64, item: Option<&str>) -> Result<(), Error> {
        match self.meter.tick(done, item) {
            Draw::Redraw(tick) => self.err.redraw(&tick.live())?,
            Draw::Heartbeat(tick) => self.err.emit(&tick.heartbeat())?,
            Draw::Event(beat) => self.event(&beat)?,
            Draw::Nothing => {}
        }
        Ok(())
    }

    /// The stage ends: a live line is taken back, so the finished line takes its place.
    pub(crate) fn end(&mut self) -> Result<(), Error> {
        if self.meter.end() {
            self.err.erase()?;
        }
        Ok(())
    }

    /// Time since the last call, or since the run began.
    pub(crate) fn lap(&mut self) -> Elapsed {
        Elapsed(self.meter.lap())
    }

    /// One NDJSON line on stderr.
    fn event(&mut self, event: &impl Serialize) -> Result<(), Error> {
        serde_json::to_writer(&mut *self.err.sink(), event).map_err(|source| Error::Json { source })?;
        writeln!(self.err.sink())?;
        Ok(())
    }

    /// The result as one JSON document, with `--json`. A command calls it exactly once, on
    /// success and on failure alike; [`Io::fail`] prints the document of a command that stopped.
    pub(crate) fn json(&mut self, result: &impl Serialize) -> Result<(), Error> {
        if self.json {
            serde_json::to_writer(&mut *self.out.sink(), result).map_err(|source| Error::Json { source })?;
            writeln!(self.out.sink())?;
        }
        Ok(())
    }

    /// A block with an event of its own: text on stderr, or the event with `--json`.
    fn block(&mut self, block: &impl Render, event: &Event) -> Result<(), Error> {
        if self.json {
            return self.event(event);
        }
        if std::mem::take(&mut self.gap) {
            self.err.emit(&Answer(Line::new()))?;
        }
        self.err.emit(block)?;
        Ok(())
    }

    /// Every diagnostic, then every notice, to stderr; returns how many errors and warnings.
    /// Errors are always shown; the rest by what the run says.
    pub(crate) fn said(
        &mut self,
        diagnostics: &[Diagnostic],
        notices: &[Notice],
        texts: Texts<'_>,
    ) -> Result<Tally, Error> {
        let mut tally = Tally::default();
        for diagnostic in diagnostics {
            let severity = diagnostic.severity();
            match severity {
                Severity::Error => tally.errors += 1,
                Severity::Warning => tally.warnings += 1,
                _ => {}
            }
            if severity == Severity::Error || self.say >= Say::Warnings {
                self.block(&Shown { diagnostic, texts }, &Event::diagnostic(diagnostic, texts))?;
            }
        }
        if self.say >= Say::Notices {
            for notice in notices {
                self.block(&Noted(notice), &Event::notice(notice))?;
            }
        }
        Ok(tally)
    }

    /// Writes the block of an error that stopped a command: text on stderr, or the one document
    /// on stdout with `--json`.
    pub(crate) fn failed(&mut self, failure: &Failure) -> Result<(), Error> {
        self.end()?;
        if self.json {
            return self.json(failure);
        }
        self.err.emit(failure)?;
        Ok(())
    }

    /// Reports `error` and its causes and returns its exit code: on stderr, a refused folder as
    /// its diagnostic; with `--json`, as the one document on stdout.
    pub(crate) fn fail(&mut self, error: &Error) -> Exit {
        let exit = error.exit();
        let written = match error {
            Error::Source(SourceError::Refused(refused)) if !self.json => {
                self.said(&[Diagnostic::from(refused)], &[], Texts::NONE).map(|_| ())
            }
            _ => self.failed(&Failure::of(error, exit)),
        };
        // Nothing is left to report a failed report to.
        match written {
            Ok(()) => exit,
            Err(_) => Exit::Usage,
        }
    }
}

/// A path as JSON: its text, any part that is not UTF-8 written as U+FFFD, so the document is
/// always valid.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Lossy<'a>(pub(crate) &'a Path);

impl Serialize for Lossy<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string_lossy())
    }
}
