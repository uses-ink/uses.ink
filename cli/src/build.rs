//! `ink build` and `ink check`.

use std::path::Path;

use model::DeployId;
use serde::Serialize;

use crate::args::{Build, Check};
use crate::error::Error;
use crate::exit::Exit;
use crate::out::OutFolder;
use crate::output::{Io, Strict};
use crate::report::Texts;
use crate::site::{Address, DEFAULT_OUT, Source};

/// What `ink build --json` prints.
#[derive(Debug, Serialize)]
struct Built<'a> {
    schema: &'static str,
    deploy: DeployId,
    out: &'a Path,
    entries: usize,
}

/// What `ink check --json` prints.
#[derive(Debug, Serialize)]
struct Checked {
    schema: &'static str,
    pages: usize,
    decisions: usize,
    errors: usize,
    warnings: usize,
}

impl Build {
    /// Refuses an output folder it will not clear before reading anything, then reads,
    /// interprets and renders the whole site; only a build that ends with [`Exit::Done`] touches
    /// the output folder.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let address = Address::from_flag(self.base.as_deref())?;
        let out = self.out.clone().unwrap_or_else(|| self.dir.join(DEFAULT_OUT));
        let folder = OutFolder::new(&out, &self.dir)?;
        folder.check()?;
        let mut source = Source::new(&self.dir);
        source.output.extend(folder.inside(&self.dir).filter(|inside| !source.output.contains(inside)));
        let loaded = source.load()?;
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let rendered = loaded.render(&address);
        let tally = io.said(&rendered.diagnostics, &rendered.notices, texts)?;
        let exit = tally.exit(Strict::from(self.strict));
        let Some(deploy) = rendered.deploy.as_ref().filter(|_| exit == Exit::Done) else {
            return Ok(if exit == Exit::Done { Exit::Content } else { exit });
        };
        folder.replace(deploy, &rendered.blobs, &loaded.store)?;
        let id = deploy.id();
        io.line(id)?;
        io.json(&Built { schema: "uses.build/v1", deploy: id, out: &out, entries: deploy.entries().len() })?;
        Ok(exit)
    }
}

impl Check {
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let loaded = Source::new(&self.dir).load()?;
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let understood = &loaded.understood;
        let tally = io.said(&understood.diagnostics, &understood.notices, texts)?;
        let pages = understood.site.as_ref().map_or(0, |site| site.pages().len());
        let decisions = understood.decisions.len();
        io.line(format_args!(
            "{pages} pages, {decisions} decisions, {} errors, {} warnings",
            tally.errors, tally.warnings
        ))?;
        io.json(&Checked {
            schema: "uses.check/v1",
            pages,
            decisions,
            errors: tally.errors,
            warnings: tally.warnings,
        })?;
        Ok(tally.exit(Strict::from(self.strict)))
    }
}
