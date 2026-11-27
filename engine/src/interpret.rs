//! Interpret: the stages in order (architecture.md section 9), each recording into one journal,
//! then the site from their decisions.
//!
//! `site.root`, `site.layout`, `file.ignore`, reading, then per page `page.publish`,
//! `page.kind`, `page.title`, `page.date`, `page.updated`, `page.slug`, `page.tags`,
//! `page.summary`, then link resolution and embeds, `page.publish` for plain files, `site.home`,
//! `site.title`, `site.description`, `site.authors`, `site.language`, `site.nav`, routes, and
//! the site. Every stage runs, so every problem is reported; the site is assembled only from a
//! journal without an error.

mod decide;
mod file;
mod journal;
mod link;
mod page;
mod site;
mod stage;
mod tree;

use model::code::AnyDecision;
use model::{Diagnostic, Notice, Snapshot, Subject};

use crate::interpret::file::read::Split;
use crate::interpret::file::{Assets, Files, Read};
use crate::interpret::journal::Journal;
use crate::interpret::link::{Aliases, Embeds, Links, Settings, Targets};
use crate::interpret::page::Pages;
use crate::interpret::site::{Assemble, Parts, Routing, Site};
use crate::interpret::stage::Stage;
use crate::interpret::tree::{Layout, Root};
use crate::{Blobs, Engine, Formats, Pins};

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
}

impl Understood {
    /// The decisions about `subject`, or every decision, in the order they were made: what
    /// `ink explain [path]` shows.
    pub fn explain<'a>(&'a self, subject: Option<&'a Subject>) -> impl Iterator<Item = &'a AnyDecision> {
        self.decisions.iter().filter(move |decision| subject.is_none_or(|subject| &decision.subject() == subject))
    }
}

impl<B: Blobs, P: Pins, F: Formats, T> Engine<B, P, F, T> {
    /// Interprets `snapshot`: decides everything, then builds the site from the decisions.
    pub fn interpret(&self, snapshot: &Snapshot) -> Understood {
        let mut journal = Journal::default();
        let pins = &self.pins;
        let scope = Root { formats: &self.formats, pins }.run(snapshot, &mut journal);
        let selection = Layout { pins }.run(&scope, &mut journal);
        let kept = Files { pins }.run(&scope, &mut journal);
        let kept_paths = kept.iter().map(|file| file.entry.path.clone()).collect();
        let Split { mut sources, files } = Read { blobs: &self.blobs, formats: &self.formats }.run(kept, &mut journal);
        let published = Pages { pins, selection }.run(&sources, &mut journal);
        let pages = sources.iter().map(|source| source.path.clone()).collect();
        let aliases: Aliases = sources.iter().collect();
        let attachments = Settings { blobs: &self.blobs }.run(&scope, &mut journal);
        let targets = Targets { scope: &scope, kept: kept_paths, pages, published, aliases, attachments };
        let used = Links { targets: &targets }.run(&mut sources, &mut journal);
        Embeds { published: &targets.published }.run(&mut sources, &mut journal);
        let published = targets.published;
        let assets = Assets { pins, used: &used }.run(&files, &mut journal);
        let home = Site { pins, scope: &scope, published: &published }.run(&sources, &mut journal);
        let shown: Vec<_> = sources.iter().filter(|source| published.contains(&source.path)).collect();
        let routes = Routing { home: home.as_ref(), assets: &assets }.run(&shown, &mut journal);
        let parts = Parts { scope: &scope, sources, assets: &assets, routes: &routes };
        let site = Assemble.run(parts, &mut journal);
        let site = site.filter(|_| !journal.stops());
        let (decisions, diagnostics, notices) = journal.into_parts();
        Understood { decisions, diagnostics, notices, site }
    }
}
