//! Where a command writes: its result to stdout, everything else to stderr.

use std::fmt;
use std::io::Write;
use std::path::Path;

use model::{Diagnostic, Notice, Severity};
use serde::Serialize;

use crate::error::Error;
use crate::exit::Exit;
use crate::report::{Event, Noted, Shown, Texts};

/// A command's two streams and whether it speaks JSON.
pub(crate) struct Io<'a> {
    out: &'a mut dyn Write,
    err: &'a mut dyn Write,
    json: bool,
    level: Level,
}

/// What reaches stderr besides errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Level {
    /// Warnings and errors: the default of a one-shot command.
    Warning,
    /// Notices too (`-v`).
    Notice,
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
    pub(crate) fn new(out: &'a mut dyn Write, err: &'a mut dyn Write, json: bool, level: Level) -> Self {
        Self { out, err, json, level }
    }

    /// Whether results are JSON.
    pub(crate) fn is_json(&self) -> bool {
        self.json
    }

    /// The result, as a line of text, or nothing with `--json` (see [`Io::json`]).
    pub(crate) fn line(&mut self, line: impl fmt::Display) -> Result<(), Error> {
        if !self.json {
            writeln!(self.out, "{line}")?;
        }
        Ok(())
    }

    /// The result as one JSON document, with `--json`. A command calls it exactly once, on
    /// success and on failure alike; [`Io::fail`] prints the document of a command that stopped.
    pub(crate) fn json(&mut self, result: &impl Serialize) -> Result<(), Error> {
        if self.json {
            serde_json::to_writer(&mut *self.out, result).map_err(|source| Error::Json { source })?;
            writeln!(self.out)?;
        }
        Ok(())
    }

    /// Text written to stdout as is, JSON or not: `ink explain --pin` without `--json`.
    pub(crate) fn raw(&mut self, text: &str) -> Result<(), Error> {
        self.out.write_all(text.as_bytes())?;
        Ok(())
    }

    /// Every diagnostic, then every notice, to stderr; returns how many errors and warnings.
    pub(crate) fn said(
        &mut self,
        diagnostics: &[Diagnostic],
        notices: &[Notice],
        texts: Texts<'_>,
    ) -> Result<Tally, Error> {
        let mut tally = Tally::default();
        for diagnostic in diagnostics {
            match diagnostic.severity() {
                Severity::Error => tally.errors += 1,
                Severity::Warning => tally.warnings += 1,
                _ => {}
            }
            if self.json {
                serde_json::to_writer(&mut *self.err, &Event::diagnostic(diagnostic, texts))
                    .map_err(|source| Error::Json { source })?;
                writeln!(self.err)?;
            } else {
                writeln!(self.err, "{}", Shown { diagnostic, texts })?;
            }
        }
        let notices = if self.level == Level::Notice { notices } else { &[] };
        for notice in notices {
            if self.json {
                serde_json::to_writer(&mut *self.err, &Event::notice(notice))
                    .map_err(|source| Error::Json { source })?;
                writeln!(self.err)?;
            } else {
                writeln!(self.err, "{}", Noted(notice))?;
            }
        }
        Ok(tally)
    }

    /// Reports `error` and its causes and returns its exit code: on stderr, a refused folder as
    /// its diagnostic; with `--json`, as the one document on stdout.
    pub(crate) fn fail(&mut self, error: &Error) -> Exit {
        let exit = error.exit();
        let written = match error {
            _ if self.json => self.json(&Failed::of(error, exit)),
            Error::Refused(refused) => self.said(&[Diagnostic::from(refused)], &[], Texts::NONE).map(|_| ()),
            other => Chain(other).write(self.err),
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

/// What every command prints on stdout with `--json` when it stops on an error.
#[derive(Debug, Serialize)]
pub(crate) struct Failed {
    schema: &'static str,
    exit: u8,
    /// The error's kind: a diagnostic code for a refused folder, `output.foreign` and the like
    /// for the rest.
    error: &'static str,
    what: String,
    /// Each cause, outermost first.
    causes: Vec<String>,
    /// The refused folder's diagnostic, when it is one.
    diagnostic: Option<Event>,
}

impl Failed {
    const SCHEMA: &'static str = "uses.error/v1";

    pub(crate) fn of(error: &Error, exit: Exit) -> Self {
        let mut causes = Vec::new();
        let mut cause = std::error::Error::source(error);
        while let Some(next) = cause {
            causes.push(next.to_string());
            cause = next.source();
        }
        let diagnostic = match error {
            Error::Refused(refused) => Some(Event::diagnostic(&Diagnostic::from(refused), Texts::NONE)),
            _ => None,
        };
        Self {
            schema: Self::SCHEMA,
            exit: exit.code(),
            error: error.kind(),
            what: error.to_string(),
            causes,
            diagnostic,
        }
    }

    /// A command line clap refused, under `--json`.
    pub(crate) fn usage(error: &clap::Error) -> Self {
        let text = error.to_string();
        let what = text.lines().next().unwrap_or_default();
        let what = what.strip_prefix("error: ").unwrap_or(what).to_owned();
        Self {
            schema: Self::SCHEMA,
            exit: Exit::Usage.code(),
            error: "cli.usage",
            what,
            causes: Vec::new(),
            diagnostic: None,
        }
    }
}

/// An error and its `source()` chain, one `caused by` line each.
#[derive(Clone, Copy, Debug)]
struct Chain<'a>(&'a Error);

impl Chain<'_> {
    fn write(self, err: &mut dyn Write) -> Result<(), Error> {
        writeln!(err, "error: {}", self.0)?;
        let mut cause = std::error::Error::source(self.0);
        while let Some(error) = cause {
            writeln!(err, "  caused by: {error}")?;
            cause = error.source();
        }
        Ok(())
    }
}
