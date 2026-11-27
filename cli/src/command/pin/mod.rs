//! `ink pin [path] <code>`: one decision's current value written into `ink.toml` as a pin,
//! keeping everything else in the file (decision 0042). The pin is printed; `--dry-run` writes
//! nothing.

pub(crate) mod pins;

use std::path::PathBuf;

use serde::Serialize;

use crate::command::explain::asked::Asked;
use crate::command::source::Source;
use crate::folder::replace::Replacing;
use crate::grammar::args::Pin;
use crate::output::error::Error;
use crate::output::exit::Exit;
use crate::output::report::Texts;
use crate::output::stage::{StageLine, Verb as Stage};
use crate::output::term::{Doc, Line, Render, Role};
use crate::output::{Io, Strict};

/// What `ink pin --json` prints.
#[derive(Debug, Serialize)]
struct Pinned<'a> {
    schema: &'static str,
    /// The pin, as a TOML table of its own.
    toml: &'a str,
    /// Whether ink.toml now holds it.
    written: bool,
}

/// A pin as a person reads it: the stage line, then the lines it adds to `ink.toml`.
#[derive(Clone, Debug)]
struct Patch<'a> {
    stage: StageLine,
    file: &'a str,
    toml: &'a str,
}

impl Render for Patch<'_> {
    fn render(&self, into: &mut Doc) {
        self.stage.render(into);
        into.push(Line::new().pad(2).with(Role::Place, format!("@@ {}", self.file)));
        for line in self.toml.trim_end().lines() {
            // a table header is what the pin is added under; every other line is added
            if line.starts_with('[') {
                into.push(Line::new().pad(3).with(Role::Plain, line));
            } else {
                into.push(Line::new().pad(2).with(Role::Good, "+").with(Role::Plain, line));
            }
        }
    }
}

impl Pin {
    /// The config file a pin is written into, in the site's folder.
    const FILE: &'static str = "ink.toml";

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
        let path: PathBuf = self.dir.join(Self::FILE);
        if !self.dry_run {
            let existing = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(source) => return Err(Error::Read { path, source }),
            };
            let text = config::pins_into(&existing, [decision]).map_err(|source| Error::Pin { source })?;
            let mut replacing = Replacing::default();
            replacing.push(path.clone(), text);
            replacing.commit()?;
        }
        let what = format!("{} of {}", asked.code, asked.subject);
        let stage = if self.dry_run {
            StageLine::new(Stage::WouldPin, format!("{what}; {} is unchanged", Self::FILE))
        } else {
            StageLine::new(Stage::Pinned, format!("{what} in {}", Self::FILE))
        };
        io.result(&Patch { stage, file: Self::FILE, toml: &pin })?;
        io.json(&Pinned { schema: "uses.pin/v2", toml: &pin, written: !self.dry_run })?;
        Ok(tally.exit(Strict::Off))
    }
}
