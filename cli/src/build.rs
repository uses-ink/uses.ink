//! `ink build` and `ink check`.

use model::{DeployId, Manifest};
use serde::Serialize;

use crate::args::{Build, Check};
use crate::error::Error;
use crate::exit::Exit;
use crate::out::OutFolder;
use crate::output::{Io, Lossy, Strict};
use crate::report::Texts;
use crate::site::{Address, DEFAULT_OUT, Source};

/// What `ink build --json` prints, whether or not the build was written.
#[derive(Debug, Serialize)]
struct Built<'a> {
    schema: &'static str,
    /// The deploy, when the site rendered.
    deploy: Option<DeployId>,
    /// Whether the output folder now holds it: only when the build exits 0.
    written: bool,
    out: Lossy<'a>,
    entries: usize,
    errors: usize,
    warnings: usize,
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
        if let (true, Some(components)) = (self.check, &self.components) {
            return crate::component::Compare { build: &self, components }.run(io);
        }
        let address = Address::from_flag(self.base.as_deref())?;
        let out = self.out.clone().unwrap_or_else(|| self.dir.join(DEFAULT_OUT));
        let folder = OutFolder::new(&out, &self.dir)?;
        folder.check()?;
        let mut source = Source::new(&self.dir, &self.origin);
        source.output.extend(folder.inside(&self.dir).filter(|inside| !source.output.contains(inside)));
        let loaded = source.load()?;
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let rendered = loaded.render(&address);
        let tally = io.said(&rendered.diagnostics, &rendered.notices, texts)?;
        let exit = tally.exit(Strict::from(self.strict));
        let deploy = rendered.deploy.as_ref();
        let written = deploy.filter(|_| exit == Exit::Done);
        if let Some(deploy) = written {
            folder.replace(deploy, &rendered.blobs, &loaded.store)?;
            io.line(deploy.id())?;
        }
        io.json(&Built {
            schema: "uses.build/v1",
            deploy: deploy.map(Manifest::id),
            written: written.is_some(),
            out: Lossy(&out),
            entries: deploy.map_or(0, |deploy| deploy.entries().len()),
            errors: tally.errors,
            warnings: tally.warnings,
        })?;
        Ok(if written.is_none() && exit == Exit::Done { Exit::Content } else { exit })
    }
}

impl Check {
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let loaded = Source::new(&self.dir, &self.origin).load()?;
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
