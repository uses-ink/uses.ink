//! Interpret: the stages in order (architecture.md section 9), each recording into one journal,
//! then the site from their decisions.
//!
//! `site.root`, `site.layout`, the vault's settings, `file.ignore`, reading, then per page `page.publish`,
//! `page.kind`, `page.title`, `page.date`, `page.updated`, `page.slug`, `page.tags`,
//! `page.summary`, then link resolution and embeds, `page.publish` for plain files, `site.home`,
//! `site.title`, `site.description`, `site.authors`, `site.language`, `site.nav`, routes, the
//! site, and the private-source guard. Every stage runs, so every problem is reported; the site
//! is assembled only from a
//! journal without an error.

mod decide;
mod format;
mod journal;
mod pipeline;

use model::code::{AnyCode, AnyDecision};
use model::{Diagnostic, Notice, Snapshot, Subject};

pub use crate::interpret::decide::trace::{Fact, Heard, List, Marker, Reading, Rung, Standing, Trace, Witnessed};
pub use crate::interpret::format::Chosen;
pub use crate::interpret::pipeline::page::Selection;

use crate::interpret::journal::Journal;
use crate::interpret::pipeline::file::read::Split;
use crate::interpret::pipeline::file::{Assets, Files, Read};
use crate::interpret::pipeline::guard::Guard;
use crate::interpret::pipeline::link::{Aliases, Embeds, Links, Targets};
use crate::interpret::pipeline::page::Pages;
use crate::interpret::pipeline::site::{Assemble, Parts, Routing, Site};
use crate::interpret::pipeline::stage::Stage;
use crate::interpret::pipeline::tree::{Layout, Root};
use crate::interpret::pipeline::vault::Settings;
use crate::{Blobs, Engine, Formats, Pins};
use logging::Field as _;

/// What interpreting a snapshot understood.
#[derive(Debug)]
#[non_exhaustive]
pub struct Understood {
    /// Every decision, in the order they were made; per-file decisions in path order.
    pub decisions: Vec<AnyDecision>,
    /// Every problem, the format's own among them, in the order found.
    pub diagnostics: Vec<Diagnostic>,
    /// Defaults chosen and other things worth knowing, in the order said.
    pub notices: Vec<Notice>,
    /// The site, or `None` when an error stops the build: all or nothing (open.md Q6).
    pub site: Option<model::Site>,
    /// The ladder asked for by [`Engine::trace`], when its decision was made.
    pub trace: Option<Trace>,
    /// Whether the site publishes every page it does not hold back, or only the pages selected.
    pub selection: Selection,
}

impl Understood {
    /// What was understood, with the decision of the format it was read with: its decision
    /// first, its diagnostics and notices before the engine's, and its replay when that was asked for.
    #[must_use]
    pub fn with(mut self, chosen: Chosen) -> Self {
        let (decisions, diagnostics, notices, trace) = chosen.into_parts();
        self.decisions.splice(0..0, decisions);
        self.diagnostics.splice(0..0, diagnostics);
        self.notices.splice(0..0, notices);
        self.trace = trace.or(self.trace);
        self
    }

    /// The decisions about `subject`, or every decision, in the order they were made: what
    /// `ink explain [path]` shows.
    pub fn explain<'a>(&'a self, subject: Option<&'a Subject>) -> impl Iterator<Item = &'a AnyDecision> {
        self.decisions.iter().filter(move |decision| subject.is_none_or(|subject| &decision.subject() == subject))
    }
}

impl<B: Blobs, P: Pins, F: Formats, T> Engine<B, P, F, T> {
    /// Interprets `snapshot`: decides everything, then builds the site from the decisions.
    pub fn interpret(&self, snapshot: &Snapshot) -> Understood {
        self.run(snapshot, Journal::default())
    }

    /// Interprets `snapshot` as [`Engine::interpret`] does, and replays the ladder of `code` on
    /// `subject` rung by rung: what `ink explain --why` shows. The trace is `None` when that
    /// decision is never made, as for a file that is ignored.
    pub fn trace(&self, snapshot: &Snapshot, code: AnyCode, subject: Subject) -> Understood {
        self.run(snapshot, Journal::asking(code, subject))
    }

    /// Says how many decisions, diagnostics, notices and published pages a run made.
    fn said([decisions, diagnostics, notices, published]: [usize; 4]) {
        logging::debug!(
            name: logging::event::INTERPRETED,
            decisions = logging::Count::from(decisions).field(),
            diagnostics = logging::Count::from(diagnostics).field(),
            notices = logging::Count::from(notices).field(),
            published = logging::Count::from(published).field(),
        );
    }

    /// Every stage in order, recording into `journal`.
    #[allow(clippy::too_many_lines, reason = "the stages in order, each named for the log")]
    fn run(&self, snapshot: &Snapshot, mut journal: Journal) -> Understood {
        let pins = &self.pins;
        let scope =
            Root { formats: &self.formats, pins }.step((logging::Stage::Discover, "root"), snapshot, &mut journal);
        let selection = Layout { pins }.step((logging::Stage::Discover, "layout"), &scope, &mut journal);
        let vault = Settings { blobs: &self.blobs }.step((logging::Stage::Discover, "vault"), &scope, &mut journal);
        let kept =
            Files { pins, templates: &vault.templates }.step((logging::Stage::Discover, "files"), &scope, &mut journal);
        let kept_paths = kept.iter().map(|file| file.entry.path.clone()).collect();
        let Split { mut sources, files } = Read { blobs: &self.blobs, formats: &self.formats }.step(
            (logging::Stage::Source, "read"),
            kept,
            &mut journal,
        );
        let published = Pages { pins, selection }.step((logging::Stage::Decide, "pages"), &sources, &mut journal);
        let pages = sources.iter().map(|source| source.path.clone()).collect();
        let aliases: Aliases = sources.iter().collect();
        let targets =
            Targets { scope: &scope, kept: kept_paths, pages, published, aliases, attachments: vault.attachments };
        let used = Links { targets: &targets }.step((logging::Stage::Decide, "links"), &mut sources, &mut journal);
        Embeds { published: &targets.published }.step((logging::Stage::Decide, "embeds"), &mut sources, &mut journal);
        let published = targets.published;
        let assets = Assets { pins, used: &used }.step((logging::Stage::Decide, "assets"), &files, &mut journal);
        let home = Site { pins, scope: &scope, published: &published }.step(
            (logging::Stage::Decide, "site"),
            &sources,
            &mut journal,
        );
        let shown: Vec<_> = sources.iter().filter(|source| published.contains(&source.path)).collect();
        let routes = Routing { home: home.as_ref(), assets: &assets }.step(
            (logging::Stage::Decide, "routes"),
            &shown,
            &mut journal,
        );
        let parts = Parts { scope: &scope, sources, assets: &assets, routes: &routes };
        let site = Assemble.step((logging::Stage::Decide, "assemble"), parts, &mut journal);
        Guard { selection }.step((logging::Stage::Decide, "guard"), (), &mut journal);
        let site = site.filter(|_| !journal.stops());
        let (decisions, diagnostics, notices, trace) = journal.into_parts();
        Self::said([
            decisions.len(),
            diagnostics.len(),
            notices.len(),
            site.as_ref().map_or(0, |site| site.pages().len()),
        ]);
        Understood { decisions, diagnostics, notices, site, trace, selection }
    }
}
