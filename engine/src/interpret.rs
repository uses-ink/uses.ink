//! Interpret: the inferrers in order (architecture.md section 9), then the site from their
//! decisions.
//!
//! `site.root`, `site.layout`, `file.ignore`, reading, then per page `page.publish`,
//! `page.kind`, `page.title`, `page.date`, `page.updated`, `page.slug`, `page.tags`,
//! `page.summary`, then link
//! resolution, `page.publish` for plain files, `site.home`, `site.title`, `site.description`,
//! `site.authors`, `site.language`, `site.nav`, routes, and the site.

mod file;
mod ladder;
mod page;
mod record;
mod site;

use std::collections::BTreeSet;

use model::{Ability, ContentPath, Decision, Diagnostic, Layout, Notice, Site, Snapshot, Subject};

use crate::pins::Pins;
use crate::{Blobs, Format};
use file::read::{Source, Split};
use file::scope::Scope;
use page::Selection;
use record::Record;
use site::link::Targets;
use site::nav::Choice;

/// What interpreting a snapshot understood.
#[derive(Debug)]
#[non_exhaustive]
pub struct Understood {
    /// Every decision, in the order the inferrers made them; per-file decisions in path order.
    pub decisions: Vec<Decision>,
    /// Every problem, the format's own among them, in the order found.
    pub diagnostics: Vec<Diagnostic>,
    /// Defaults chosen and other things worth knowing, in the order said.
    pub notices: Vec<Notice>,
    /// The site, or `None` when an error stops the build: all or nothing (open.md Q6).
    pub site: Option<Site>,
}

/// Interprets `snapshot`: decides everything, then builds the site from the decisions.
pub fn interpret(snapshot: &Snapshot, blobs: &impl Blobs, pins: &impl Pins, formats: &[&dyn Format]) -> Understood {
    let mut record = Record::default();
    let root = site::root::decide(snapshot, formats, pins, &mut record);
    let scope = Scope::new(snapshot, root);
    let layout = site::layout::decide(&scope, pins, &mut record);
    let kept = file::ignore::decide(&scope, pins, &mut record);
    let kept_paths: BTreeSet<ContentPath> = kept.iter().map(|file| file.path().clone()).collect();
    let Split { mut sources, files } = file::read::read(kept, blobs, formats, &mut record);
    let selection = selection(snapshot, layout);
    let published: BTreeSet<ContentPath> = sources
        .iter()
        .filter(|source| page::decide(source, selection, pins, &mut record))
        .map(|source| source.path.clone())
        .collect();
    let pages = sources.iter().map(|source| source.path.clone()).collect();
    let targets = Targets { tree: snapshot.tree(), scope: &scope, kept: kept_paths, pages, published };
    let used = site::link::resolve(&mut sources, &targets, &mut record);
    let published = targets.published;
    let assets = file::asset::decide(&files, &used, pins, &mut record);
    let home = site::home::decide(&sources, &published, pins, &mut record);
    let home_source = sources.iter().find(|source| Some(&source.path) == home.as_ref());
    site::identity::decide(home_source, &scope.root, pins, &mut record);
    site::nav::decide(&Choice { sources: &sources, published: &published, home: home.as_ref() }, pins, &mut record);
    let routed_sources: Vec<&Source> = sources.iter().filter(|source| published.contains(&source.path)).collect();
    let routes = site::route::assign(&routed_sources, &assets, home.as_ref(), &mut record);
    let site = if record.stops() {
        None
    } else {
        let parts = site::assemble::Parts { scope: &scope, sources, assets: &assets, routes: &routes };
        site::assemble::site(parts, &mut record)
    };
    let Record { decisions, diagnostics, notices, .. } = record;
    let site = if diagnostics.iter().any(|d| d.severity() == model::Severity::Error) { None } else { site };
    Understood { decisions, diagnostics, notices, site }
}

/// Vaults and private sources publish nothing until selected (decision 0017).
fn selection(snapshot: &Snapshot, layout: Layout) -> Selection {
    if layout == Layout::Vault || snapshot.observed().has.contains(&Ability::Private) {
        Selection::Explicit
    } else {
        Selection::Implicit
    }
}

/// The decisions about `subject`, or every decision, in the order they were made: what
/// `ink explain [path]` shows.
pub fn explain<'a>(understood: &'a Understood, subject: Option<&Subject>) -> Vec<&'a Decision> {
    understood.decisions.iter().filter(|decision| subject.is_none_or(|subject| &decision.subject == subject)).collect()
}
