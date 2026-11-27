//! The stage resolving links. Formats leave links to files as written; once every page is
//! decided, each published page is folded, its links and images pointed at pages, anchors and
//! assets. A link that names nothing, an unpublished file, a plain file, or several files stays
//! as written and says why: nothing from an unpublished file reaches a published page. An embed
//! is resolved as a link is, then filled by [`Embeds`]; one that names no page it can show is a
//! link in a paragraph of its own.

mod embed;
mod target;
mod vault;

use std::collections::BTreeSet;

use model::{
    Anchor, Announcement, Block, ContentPath, Diagnostic, ExternalUrl, Fold, ImageSource, Inline, LinkTarget,
    MediaType, Notice, PageId, Problem, Slug, Visit,
};

pub(crate) use embed::Embeds;
pub(crate) use target::{Aliases, Targets};
use target::{Found, Written};
pub(crate) use vault::Settings;

use crate::interpret::file::asset::Uses;
use crate::interpret::file::read::Source;
use crate::interpret::journal::Journal;
use crate::interpret::stage::Stage;

/// Resolves every link of every published page, in its body and its footnotes.
#[derive(Debug)]
pub(crate) struct Links<'a> {
    pub(crate) targets: &'a Targets<'a>,
}

impl Stage<&mut [Source]> for Links<'_> {
    /// Each plain file a published page shows as an image, with the first page that does.
    type Out = Uses;

    fn run(&self, sources: &mut [Source], journal: &mut Journal) -> Uses {
        let mut used = Uses::new();
        for source in sources.iter_mut().filter(|source| self.targets.published.contains(&source.path)) {
            let mut headings = Headings::default();
            headings.document(&source.read.document);
            let mut resolve = Resolve {
                from: &source.path,
                targets: self.targets,
                anchors: headings.0,
                problems: Vec::new(),
                notices: Vec::new(),
                used: &mut used,
            };
            let document = std::mem::take(&mut source.read.document);
            let document = resolve.document(document);
            let (problems, notices) = (resolve.problems, resolve.notices);
            source.read.document = document;
            for problem in problems {
                journal.diagnose(Diagnostic::new(problem).on(source.subject().into()));
            }
            for announcement in notices {
                journal.notice(Notice::new(announcement).on(source.subject().into()));
            }
        }
        used
    }
}

/// The anchors of a page's headings and block ids, at any depth.
#[derive(Debug, Default)]
struct Headings(BTreeSet<Anchor>);

impl Visit for Headings {
    fn block(&mut self, block: &Block) {
        if let Block::Heading { anchor, .. } = block {
            self.0.insert(anchor.clone());
        }
        block.walk(self);
    }

    fn inline(&mut self, inline: &Inline) {
        if let Inline::Anchor(anchor) = inline {
            self.0.insert(anchor.clone());
        }
        inline.walk(self);
    }
}

/// One page's links being resolved.
#[derive(Debug)]
struct Resolve<'a, 'b> {
    from: &'a ContentPath,
    targets: &'a Targets<'b>,
    /// The page's own anchors, which a `#` link must name.
    anchors: BTreeSet<Anchor>,
    problems: Vec<Problem>,
    notices: Vec<Announcement>,
    used: &'a mut Uses,
}

impl Fold for Resolve<'_, '_> {
    /// An embed resolved as a link; one that names no page is a link to what it names.
    fn block(&mut self, block: Block) -> Block {
        let Block::Embed { target: LinkTarget::Unresolved { written }, blocks } = block else {
            return block.fold(self);
        };
        match self.target(Written(&written)) {
            Some(target @ (LinkTarget::Page { .. } | LinkTarget::Anchor(_))) => Block::Embed { target, blocks },
            target => {
                let target = target.unwrap_or(LinkTarget::Unresolved { written: written.clone() });
                Block::Paragraph(vec![Inline::Link { target, children: vec![Inline::Text(written)] }])
            }
        }
    }

    fn link(&mut self, target: LinkTarget, children: Vec<Inline>) -> Inline {
        let target = match target {
            LinkTarget::Unresolved { written } => {
                self.target(Written(&written)).unwrap_or(LinkTarget::Unresolved { written })
            }
            resolved => resolved,
        };
        if let LinkTarget::Anchor(anchor) = &target
            && !self.anchors.contains(anchor)
        {
            self.problems.push(Problem::LinkAnchorMissing { written: format!("#{anchor}") });
        }
        Inline::Link { target, children: self.inlines(children) }
    }

    fn image(&mut self, source: ImageSource, alt: String) -> Inline {
        let source = match source {
            ImageSource::Unresolved { written } => {
                self.picture(Written(&written)).unwrap_or(ImageSource::Unresolved { written })
            }
            resolved => resolved,
        };
        Inline::Image { source, alt }
    }
}

impl Resolve<'_, '_> {
    /// What a link points at: an anchor of this page, a link out, or a published page.
    fn target(&mut self, written: Written<'_>) -> Option<LinkTarget> {
        let (path, anchor) = written.split();
        let anchor = anchor.and_then(|anchor| Slug::from_text(anchor).ok()).map(Anchor::new);
        if path.is_empty() {
            return self.missing(written, anchor.map(LinkTarget::Anchor));
        }
        if Written(path).is_external() {
            return self.missing(written, ExternalUrl::parse(written.0).ok().map(LinkTarget::External));
        }
        match self.settle(written, self.targets.find(self.from, Written(path))) {
            Found::Page(page) => Some(LinkTarget::Page { id: PageId::of(&page), anchor }),
            Found::File(target) => self.refuse(Problem::LinkTargetAsset { written: written.into(), target }),
            found => self.unfound(written, found),
        }
    }

    /// What an image shows: a link out, or a plain file of a kind the engine serves, which a page
    /// showing it publishes.
    fn picture(&mut self, written: Written<'_>) -> Option<ImageSource> {
        if written.is_external() {
            return self.missing(written, ExternalUrl::parse(written.0).ok().map(ImageSource::External));
        }
        let (path, _) = written.split();
        match self.settle(written, self.targets.find(self.from, Written(path))) {
            Found::File(file) => match MediaType::try_from(&file) {
                Ok(_) => {
                    self.used.entry(file.clone()).or_insert_with(|| self.from.clone());
                    Some(ImageSource::Asset(file))
                }
                // never served, so never pointed at: the theme shows the alt text
                Err(unknown) => self.refuse(Problem::AssetMediaUnknown { path: unknown.path }),
            },
            Found::Page(_) => self.missing(written, None),
            found => self.unfound(written, found),
        }
    }

    /// What `found` is, a choice the attachment folder settled said as `link.target.attached`.
    fn settle(&mut self, written: Written<'_>, found: Found) -> Found {
        let Found::Settled(chosen, candidates) = found else { return found };
        if let Found::Page(target) | Found::File(target) | Found::Unpublished(target) = chosen.as_ref() {
            let target = target.clone();
            self.notices.push(Announcement::LinkTargetAttached { written: written.into(), target, candidates });
        }
        *chosen
    }

    /// The problem of a link that names no file it can show, or several.
    fn unfound<T>(&mut self, written: Written<'_>, found: Found) -> Option<T> {
        match found {
            Found::Unpublished(target) => {
                self.refuse(Problem::LinkTargetUnpublished { written: written.into(), target })
            }
            Found::Ambiguous(candidates) => {
                self.refuse(Problem::LinkTargetAmbiguous { written: written.into(), candidates })
            }
            Found::Missing | Found::Page(_) | Found::File(_) | Found::Settled(..) => self.missing(written, None),
        }
    }

    /// `resolved`, or `link.target.missing` when there is none.
    fn missing<T>(&mut self, written: Written<'_>, resolved: Option<T>) -> Option<T> {
        if resolved.is_none() {
            self.problems.push(Problem::LinkTargetMissing { written: written.into() });
        }
        resolved
    }

    /// Nothing, with `problem` said.
    fn refuse<T>(&mut self, problem: Problem) -> Option<T> {
        self.problems.push(problem);
        None
    }
}
