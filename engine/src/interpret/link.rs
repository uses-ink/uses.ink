//! The stage resolving links. Formats leave links to files as written; once every page is
//! decided, each published page is folded, its links and images pointed at pages, anchors and
//! assets. A link that names nothing, an unpublished file, a plain file, or several files stays
//! as written and says why: nothing from an unpublished file reaches a published page. An embed
//! is resolved as a link is, then filled by [`Embeds`]; one that names no page it can show is a
//! link in a paragraph of its own.

mod embed;
mod target;

use std::collections::BTreeSet;

use model::{
    Anchor, Announcement, Block, ContentPath, Diagnostic, ExternalUrl, Fold, ImageSource, Inline, LinkTarget,
    MediaType, Notice, PageId, Problem, Slug, Visit,
};

pub(crate) use embed::Embeds;
pub(crate) use target::{Aliases, Targets};
use target::{Found, Written};

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
    /// A paragraph holding an embed among its text is ended before it and started after it, with
    /// the embed a block between: blocks cannot sit inside text.
    fn blocks(&mut self, blocks: Vec<Block>) -> Vec<Block> {
        let mut folded = Vec::with_capacity(blocks.len());
        for block in blocks {
            match block {
                Block::Paragraph(inlines) if inlines.iter().any(|inline| matches!(inline, Inline::Embed { .. })) => {
                    folded.extend(self.split(inlines));
                }
                block => folded.push(self.block(block)),
            }
        }
        folded
    }

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

    /// An embed where only text fits (a heading, a table cell, a link's text) is a link to what
    /// it names, and that is said.
    fn inline(&mut self, inline: Inline) -> Inline {
        let Inline::Embed { target: LinkTarget::Unresolved { written } } = inline else { return inline.fold(self) };
        self.notices.push(Announcement::EmbedInlineLinked { written: written.clone() });
        Self::linked(self.target(Written(&written)), written)
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
    /// A link to `target`, as written when it names nothing, showing `written`.
    fn linked(target: Option<LinkTarget>, written: String) -> Inline {
        let target = target.unwrap_or_else(|| LinkTarget::Unresolved { written: written.clone() });
        Inline::Link { target, children: vec![Inline::Text(written)] }
    }

    /// The blocks of a paragraph holding embeds: its text before each embed that shows a page,
    /// the embed, and the text after. An embed naming no page it can show stays a link in the
    /// text.
    fn split(&mut self, inlines: Vec<Inline>) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut run = Vec::new();
        for inline in inlines {
            let Inline::Embed { target: LinkTarget::Unresolved { written } } = inline else {
                run.push(self.inline(inline));
                continue;
            };
            match self.target(Written(&written)) {
                Some(target @ (LinkTarget::Page { .. } | LinkTarget::Anchor(_))) => {
                    Self::paragraph(&mut run, &mut blocks);
                    blocks.push(Block::Embed { target, blocks: Vec::new() });
                }
                target => run.push(Self::linked(target, written)),
            }
        }
        Self::paragraph(&mut run, &mut blocks);
        blocks
    }

    /// The paragraph `run` makes, pushed to `blocks` when it shows anything, and emptied.
    fn paragraph(run: &mut Vec<Inline>, blocks: &mut Vec<Block>) {
        let inlines = std::mem::take(run);
        let shows = inlines.iter().any(|inline| match inline {
            Inline::Text(text) => !text.trim().is_empty(),
            Inline::Break | Inline::SoftBreak => false,
            _ => true,
        });
        if shows {
            blocks.push(Block::Paragraph(inlines));
        }
    }

    /// What a link points at: an anchor of this page, a link out, or a published page.
    fn target(&mut self, written: Written<'_>) -> Option<LinkTarget> {
        let (path, anchor) = written.split();
        if path.is_empty() {
            let anchor = anchor.and_then(|anchor| Slug::from_text(anchor).ok()).map(Anchor::new);
            return self.missing(written, anchor.map(LinkTarget::Anchor));
        }
        if self.outside(path) {
            return self.missing(written, ExternalUrl::parse(written.0).ok().map(LinkTarget::External));
        }
        let anchor = anchor.and_then(|text| self.anchored(text));
        match self.settle(written, self.targets.find(self.from, Written(path))) {
            Found::Page(page) => Some(LinkTarget::Page { id: PageId::of(&page), anchor }),
            Found::File(target) => self.refuse(Problem::LinkTargetAsset { written: written.into(), target }),
            found => self.unfound(written, found),
        }
    }

    /// Whether `path` is a link out: shaped like a URL (`https:`) and naming no file of the
    /// vault, which is looked up first since a file name may hold a colon (`meet: notes.md`).
    fn outside(&self, path: &str) -> bool {
        Written(path).is_external() && matches!(self.targets.find(self.from, Written(path)), Found::Missing)
    }

    /// The anchor a link's text after `#` names: none when it is empty, and `link.anchor.missing`
    /// when it has no letter or digit to make an anchor of.
    fn anchored(&mut self, text: &str) -> Option<Anchor> {
        match Slug::from_text(text) {
            Ok(slug) => Some(Anchor::new(slug)),
            Err(_) if text.trim().is_empty() => None,
            Err(_) => {
                self.problems.push(Problem::LinkAnchorMissing { written: format!("#{text}") });
                None
            }
        }
    }

    /// What an image shows: a link out, or a plain file of a kind the engine serves, which a page
    /// showing it publishes.
    fn picture(&mut self, written: Written<'_>) -> Option<ImageSource> {
        let (path, _) = written.split();
        if self.outside(path) {
            return self.missing(written, ExternalUrl::parse(written.0).ok().map(ImageSource::External));
        }
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
