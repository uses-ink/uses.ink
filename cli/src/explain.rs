//! `ink explain [--pin] [path]`: the decisions behind the site, with their evidence, or every
//! decision as `ink.toml` pins.

use std::fmt;

use model::{ContentPath, Decision, Evidence, Subject};
use serde::Serialize;

use crate::args::Explain;
use crate::error::Error;
use crate::exit::Exit;
use crate::output::{Io, Strict};
use crate::report::Texts;
use crate::site::Source;

/// What `ink explain --json` prints.
#[derive(Debug, Serialize)]
struct Explained<'a> {
    schema: &'static str,
    decisions: Vec<&'a Decision>,
}

/// What `ink explain --pin --json` prints.
#[derive(Debug, Serialize)]
struct Pinned {
    schema: &'static str,
    toml: String,
}

impl Explain {
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let subject = match &self.path {
            Some(written) => {
                let path: ContentPath =
                    written.parse().map_err(|source| Error::Path { written: written.clone(), source })?;
                Some(Subject::File(path))
            }
            None => None,
        };
        let loaded = Source::new(&self.dir).load()?;
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let understood = &loaded.understood;
        let tally = io.said(&understood.diagnostics, &understood.notices, texts)?;
        if self.pin {
            let toml = config::pins(&understood.decisions);
            if io.is_json() {
                io.json(&Pinned { schema: "uses.pin/v1", toml })?;
            } else {
                io.raw(&toml)?;
            }
            return Ok(tally.exit(Strict::Off));
        }
        // HACK(the CLI is ported onto typed decisions, roadmap phase 3): `Line` and the JSON
        // still read the untyped decision, converted from the erased one without loss.
        let decisions: Vec<Decision> = understood.explain(subject.as_ref()).cloned().map(Decision::from).collect();
        let mut current: Option<&Subject> = None;
        for decision in &decisions {
            if current != Some(&decision.subject) {
                io.line(&decision.subject)?;
                current = Some(&decision.subject);
            }
            io.line(Line(decision))?;
        }
        io.json(&Explained { schema: "uses.explain/v1", decisions: decisions.iter().collect() })?;
        Ok(tally.exit(Strict::Off))
    }
}

/// One decision: `  code  value  from evidence`.
#[derive(Clone, Copy, Debug)]
struct Line<'a>(&'a Decision);

impl fmt::Display for Line<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = Value(self.0).to_string();
        write!(out, "  {:<18} {value:<24} {}", self.0.code().name(), Because(&self.0.evidence))
    }
}

/// A decided value as text: strings bare, nothing as `none`, the rest as compact JSON.
#[derive(Clone, Copy, Debug)]
struct Value<'a>(&'a Decision);

impl fmt::Display for Value<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        // `Decided` serializes as `{ "<code>": value }`.
        let json = serde_json::to_value(&self.0.decided).unwrap_or(serde_json::Value::Null);
        let inner = json.as_object().and_then(|object| object.values().next()).unwrap_or(&json);
        match inner {
            serde_json::Value::String(text) => out.write_str(text),
            serde_json::Value::Null => out.write_str("none"),
            other => write!(out, "{other}"),
        }
    }
}

/// Where a decision came from.
#[derive(Clone, Copy, Debug)]
struct Because<'a>(&'a Evidence);

impl fmt::Display for Because<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Evidence::Frontmatter { path, span } => write!(out, "from the frontmatter of {path}, {span}"),
            Evidence::Filename { path } => write!(out, "from the name of {path}"),
            Evidence::Heading { path, span } => write!(out, "from the heading of {path}, {span}"),
            Evidence::History { .. } => write!(out, "from the history"),
            Evidence::Body { path } => write!(out, "from the text of {path}"),
            Evidence::Marker { path } => write!(out, "because {path} is there"),
            Evidence::Config { span } => write!(out, "pinned in ink.toml, {span}"),
            Evidence::Default { .. } => write!(out, "by default"),
            _ => write!(out, "from evidence this version cannot show"),
        }
    }
}
