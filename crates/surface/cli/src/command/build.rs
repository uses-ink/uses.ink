//! `ink build` and `ink check`.

use engine::{Compared, Host};
use model::record::{Build as Record, Changes};
use model::{Announcement, DeployId, Manifest, Notice};
use serde::Serialize;

use crate::command::change::{Listed, Scope};
use crate::command::images::Watched;
use crate::command::source::{Address, DEFAULT_OUT, Loaded, Rendering, Source};
use crate::folder::cache::Caching;
use crate::folder::written::{Baseline, Deployed, OutFolder};
use crate::grammar::args::{Build, Check};
use crate::output::error::Error;
use crate::output::exit::Exit;
use crate::output::report::Texts;
use crate::output::stage::{Counted, Plural, StageLine, Verb};
use crate::output::term::Answer;
use crate::output::{Io, Lossy, Say, Strict};

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

/// What a finished build says: how many files went where, and how many warnings it had.
#[derive(Clone, Copy, Debug)]
struct Summary<'a> {
    files: usize,
    root: &'a std::path::Path,
    warnings: usize,
}

impl std::fmt::Display for Summary<'_> {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "{} in {}", Plural(self.files, "file"), self.root.display())?;
        match self.warnings {
            0 => Ok(()),
            count => write!(out, ", {}", Plural(count, "warning")),
        }
    }
}

impl Build {
    /// Refuses an output folder it will not clear before reading anything, then reads,
    /// interprets and renders the whole site; only a build that ends with [`Exit::Done`] touches
    /// the output folder.
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        if self.check {
            return self.compare(io);
        }
        let address = Address::from_flag(self.base.as_deref())?;
        let out = self.out.clone().unwrap_or_else(|| self.dir.join(DEFAULT_OUT));
        let (folder, baseline) = self.folder(&out)?;
        let loaded = self.loaded(&folder)?;
        loaded.staged(io)?;
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let mut rendered = self.render(&loaded, &address, io)?;
        if rendered.deploy.is_some() {
            loaded.rendered(io)?;
        }
        let record = loaded.record();
        let compared = baseline.compare(&loaded, &record, &mut rendered);
        let tally = io.said(&rendered.diagnostics, &rendered.notices, texts)?;
        let changes = compared.as_ref().map(|compared| &compared.changes);
        changes.map_or(Ok(()), |changes| Self::listed(io, changes, folder.root()))?;
        let exit = tally.exit(Strict::from(self.strict));
        let deploy = rendered.deploy.as_ref();
        let written = deploy.filter(|_| exit == Exit::Done);
        if let Some(deploy) = written {
            folder.replace(Deployed { manifest: deploy, blobs: &rendered.blobs, record: &record }, &loaded.store)?;
            let said = Summary { files: deploy.entries().len(), root: folder.root(), warnings: tally.warnings };
            let took = io.lap();
            io.stage(&StageLine::new(Verb::Built, said).closing(tally).took(took))?;
            io.result(&Answer::of(deploy.id()))?;
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
    /// `--check`: the build compared with the build of the components it carries.
    fn compare(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let components = crate::command::bundle::Components::new(self.components.as_deref());
        crate::command::component::Compare { build: &self, components }.run(io)
    }

    /// The output folder `out`, once it is known to be one the build may clear, and what it holds.
    fn folder(&self, out: &std::path::Path) -> Result<(OutFolder, Baseline), Error> {
        let folder = OutFolder::new(out, &self.dir)?;
        folder.check()?;
        let baseline = folder.baseline()?;
        Ok((folder, baseline))
    }

    /// The site read, with the output folder left out of it.
    fn loaded(&self, folder: &OutFolder) -> Result<crate::command::source::Loaded, Error> {
        let mut source = Source::new(&self.dir, &self.origin);
        source.output.extend(folder.inside(&self.dir).filter(|inside| !source.output.contains(inside)));
        source.load()
    }

    /// The change set, all of it when the run says stages and the newly public part otherwise.
    fn listed(io: &mut Io<'_>, changes: &model::record::Changes, out: &std::path::Path) -> Result<(), Error> {
        let scope = if io.says(Say::Stages) { Scope::Whole } else { Scope::Public };
        io.aside(Say::Warnings, &Listed { changes, out, scope })
    }

    /// The site rendered through the image and page caches, which say what they did, and the
    /// image work, which says how far it has got.
    fn render(&self, loaded: &Loaded, address: &Address, io: &mut Io<'_>) -> Result<Rendering, Error> {
        let caching = Caching::new(&self.dir, self.cache.as_deref());
        let images = caching.pixels(loaded.config.image());
        let watched = Watched::new(&images, io);
        let mut rendered = loaded.render(address, Host::new(&watched).cached(caching.disk()));
        watched.finish()?;
        rendered.notices.extend(caching.finish());
        Ok(rendered)
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
        let said = format!("{}, {}: {}", Plural(pages, "page"), Plural(decisions, "decision"), Counted(tally));
        io.stage(&StageLine::new(Verb::Checked, said).closing(tally))?;
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
