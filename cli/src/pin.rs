//! `ink pin [path] <code>`: one decision's current value written into `ink.toml` as a pin,
//! keeping everything else in the file (decision 0042). The pin is printed; `--dry-run` writes
//! nothing.

use std::fmt;
use std::path::PathBuf;

use serde::Serialize;

use crate::args::Pin;
use crate::asked::Asked;
use crate::error::Error;
use crate::exit::Exit;
use crate::output::{Io, Strict};
use crate::report::Texts;
use crate::site::Source;

/// What `ink pin --json` prints.
#[derive(Debug, Serialize)]
struct Pinned<'a> {
    schema: &'static str,
    /// The pin, as a TOML table of its own.
    toml: &'a str,
    /// Whether ink.toml now holds it.
    written: bool,
}

/// What happened to `ink.toml`.
#[derive(Clone, Copy, Debug)]
enum Outcome<'a> {
    Written(&'a std::path::Path),
    Dry,
}

impl fmt::Display for Outcome<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Written(path) => write!(out, "wrote the pin into {}", path.display()),
            Self::Dry => out.write_str("dry run: nothing written"),
        }
    }
}

impl Pin {
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let asked = Asked::words(&self.words)?;
        let loaded = Source::new(&self.dir, &self.origin).load()?;
        asked.within(&loaded.snapshot)?;
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let understood = &loaded.understood;
        let tally = io.said(&understood.diagnostics, &understood.notices, texts)?;
        let decision = understood
            .decisions
            .iter()
            .find(|decision| decision.code() == asked.code && decision.subject() == asked.subject)
            .ok_or_else(|| Error::Undecided { code: asked.code, subject: asked.subject.clone() })?;
        let pin = config::pins_into("", [decision]).map_err(|source| Error::Pin { source })?;
        let path: PathBuf = self.dir.join("ink.toml");
        let outcome = if self.dry_run {
            Outcome::Dry
        } else {
            let existing = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(source) => return Err(Error::Read { path, source }),
            };
            let text = config::pins_into(&existing, [decision]).map_err(|source| Error::Pin { source })?;
            std::fs::write(&path, text).map_err(|source| Error::Write { path: path.clone(), source })?;
            Outcome::Written(&path)
        };
        io.line(pin.trim_end())?;
        io.line(outcome)?;
        io.json(&Pinned { schema: "uses.pin/v2", toml: &pin, written: matches!(outcome, Outcome::Written(_)) })?;
        Ok(tally.exit(Strict::Off))
    }
}
