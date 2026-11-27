//! `ink build` and `ink check`.

use engine::{Compared, Host};
use model::record::{Build as Record, Changes};
use model::{Announcement, DeployId, Manifest, Notice};
use serde::Serialize;

use crate::args::{Build, Check};
use crate::cache::Caching;
use crate::change::Listed;
use crate::error::Error;
use crate::exit::Exit;
use crate::out::{Baseline, Deployed, OutFolder};
use crate::output::{Io, Lossy, Strict};
use crate::report::Texts;
use crate::site::{Address, DEFAULT_OUT, Loaded, Rendering, Source};

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
    /// What changed against the previous build in the output folder, when the site was
    /// interpreted.
    changes: Option<&'a Changes>,
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
        let baseline = folder.baseline()?;
        let mut source = Source::new(&self.dir, &self.origin);
        source.output.extend(folder.inside(&self.dir).filter(|inside| !source.output.contains(inside)));
        let loaded = source.load()?;
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let mut rendered = self.render(&loaded, &address);
        let record = loaded.record();
        let compared = baseline.compare(&loaded, &record, &mut rendered);
        let tally = io.said(&rendered.diagnostics, &rendered.notices, texts)?;
        let changes = compared.as_ref().map(|compared| &compared.changes);
        if let Some(changes) = changes {
            io.aside(Listed { changes, out: folder.root() })?;
        }
        let exit = tally.exit(Strict::from(self.strict));
        let deploy = rendered.deploy.as_ref();
        let written = deploy.filter(|_| exit == Exit::Done);
        if let Some(deploy) = written {
            folder.replace(Deployed { manifest: deploy, blobs: &rendered.blobs, record: &record }, &loaded.store)?;
            io.line(deploy.id())?;
        }
        io.json(&Built {
            schema: "uses.build/v2",
            deploy: deploy.map(Manifest::id),
            written: written.is_some(),
            out: Lossy(&out),
            entries: deploy.map_or(0, |deploy| deploy.entries().len()),
            changes,
            errors: tally.errors,
            warnings: tally.warnings,
        })?;
        Ok(if written.is_none() && exit == Exit::Done { Exit::Content } else { exit })
    }
}

impl Build {
    /// The site rendered through the image and page caches, which say what they did.
    fn render(&self, loaded: &Loaded, address: &Address) -> Rendering {
        let caching = Caching::new(&self.dir, self.cache.as_deref());
        let images = caching.pixels();
        let mut rendered = loaded.render(address, Host::new(&images).cached(caching.disk()));
        rendered.notices.extend(caching.finish());
        rendered
    }
}

impl Baseline {
    /// `record` compared with this baseline, when the site was interpreted; what that calls
    /// for said into `rendered`, so it is printed and counted with the rest.
    fn compare(&self, loaded: &Loaded, record: &Record, rendered: &mut Rendering) -> Option<Compared> {
        let compared =
            loaded.understood.site.as_ref().map(|_| Compared::of(self.against.clone(), self.record.as_ref(), record));
        if let Some(compared) = &compared {
            rendered.diagnostics.extend(compared.diagnostics.iter().cloned());
        }
        if let Some(error) = &self.unread {
            rendered.notices.push(Notice::new(Announcement::BuildRecordUnread { reason: error.to_string() }));
        }
        compared
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
