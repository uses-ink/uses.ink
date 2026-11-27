//! The stage filling embeds. Once every published page's links are resolved, each embed of a
//! page (`![[Note]]`, `![[Note#Heading]]`, `![[Note#^block]]`) is given the blocks it shows: the
//! page's, the section under the heading, or the block holding the anchor. Embeds inside what is
//! shown are filled in turn. An embed that would show itself again, through any chain of embeds,
//! is `embed.target.cycle`, one past the budget is `embed.budget.count`, one naming no part of the
//! page is `embed.anchor.missing`; each is shown as a link instead.
//!
//! What is shown keeps pointing where it did on its own page: a link to an anchor of that page
//! points at the page, and a footnote reference, whose note stays on that page, is removed
//! (`embed.footnote.dropped`). A heading it shows keeps its text but not necessarily its anchor:
//! an id may appear once on a page, so an anchor the page already has, or an earlier embed
//! showed, becomes `anchor-1`, `anchor-2`, in document order.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use model::{
    Anchor, Announcement, Block, ContentPath, Diagnostic, Document, Fold, Inline, LinkTarget, Notice, PageId, Problem,
    Slug, Subject, Visit,
};

use crate::interpret::file::read::Source;
use crate::interpret::journal::Journal;
use crate::interpret::stage::Stage;

use super::Headings;

/// Embeds a page may fill, embeds inside embeds counted.
const BUDGET: usize = 64;

/// Fills the embeds of every published page.
#[derive(Debug)]
pub(crate) struct Embeds<'a> {
    pub(crate) published: &'a std::collections::BTreeSet<ContentPath>,
}

impl Stage<&mut [Source]> for Embeds<'_> {
    type Out = ();

    fn run(&self, sources: &mut [Source], journal: &mut Journal) {
        let pages: BTreeMap<PageId, (ContentPath, Document)> = sources
            .iter()
            .filter(|source| self.published.contains(&source.path))
            .map(|source| (PageId::of(&source.path), (source.path.clone(), source.read.document.clone())))
            .collect();
        for source in sources.iter_mut().filter(|source| self.published.contains(&source.path)) {
            let id = PageId::of(&source.path);
            let mut taken = Headings::default();
            taken.document(&source.read.document);
            let mut fill =
                Fill { pages: &pages, shown: vec![(id, None)], budget: BUDGET, said: Vec::new(), taken: taken.0 };
            let document = std::mem::take(&mut source.read.document);
            source.read.document = fill.document(document);
            let subject = Subject::from(source.subject());
            for said in fill.said {
                match said {
                    Said::Problem(problem) => journal.diagnose(Diagnostic::new(problem).on(subject.clone())),
                    Said::Notice(announcement) => journal.notice(Notice::new(announcement).on(subject.clone())),
                }
            }
        }
    }
}

/// What filling one page's embeds found worth saying.
#[derive(Debug)]
enum Said {
    Problem(Problem),
    Notice(Announcement),
}

/// One page's embeds being filled.
#[derive(Debug)]
struct Fill<'a> {
    /// Every published page, by its id: its path and its document with links resolved.
    pages: &'a BTreeMap<PageId, (ContentPath, Document)>,
    /// The page and part being shown at each depth, outermost first: the page itself, then each
    /// embed inside the one before.
    shown: Vec<(PageId, Option<Anchor>)>,
    /// Embeds still allowed.
    budget: usize,
    said: Vec<Said>,
    /// Every anchor on the page: its own, then each one an embed has shown.
    taken: BTreeSet<Anchor>,
}

/// A page and part as a person reads it: `notes/a.md#intro`.
#[derive(Clone, Copy, Debug)]
struct Named<'a>(&'a ContentPath, Option<&'a Anchor>);

impl fmt::Display for Named<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.1 {
            Some(anchor) => write!(out, "{}#{anchor}", self.0),
            None => write!(out, "{}", self.0),
        }
    }
}

impl Fold for Fill<'_> {
    fn block(&mut self, block: Block) -> Block {
        match block {
            Block::Embed { target: LinkTarget::Page { id, anchor }, .. } => self.embed(id, anchor),
            Block::Embed { target: LinkTarget::Anchor(anchor), .. } => {
                let id = self.shown.last().map(|(id, _)| *id);
                match id {
                    Some(id) => self.embed(id, Some(anchor)),
                    None => Block::Embed { target: LinkTarget::Anchor(anchor), blocks: Vec::new() },
                }
            }
            block => block.fold(self),
        }
    }
}

impl Fill<'_> {
    /// The embed of `anchor`'s part of the page `id`, filled, or a link to it when it cannot be.
    fn embed(&mut self, id: PageId, anchor: Option<Anchor>) -> Block {
        let Some((path, document)) = self.pages.get(&id) else {
            return Block::Embed { target: LinkTarget::Page { id, anchor }, blocks: Vec::new() };
        };
        let written = Named(path, anchor.as_ref()).to_string();
        let link = |written: String| {
            let target = LinkTarget::Page { id, anchor: anchor.clone() };
            Block::Paragraph(vec![Inline::Link { target, children: vec![Inline::Text(written)] }])
        };
        if self.shown.iter().any(|(shown, part)| *shown == id && *part == anchor) {
            let files: Vec<ContentPath> = self
                .shown
                .iter()
                .filter_map(|(shown, _)| self.pages.get(shown))
                .map(|(path, _)| path.clone())
                .collect();
            self.said.push(Said::Problem(Problem::EmbedTargetCycle { written: written.clone(), files }));
            return link(written);
        }
        if self.budget == 0 {
            self.said.push(Said::Problem(Problem::EmbedBudgetCount { written: written.clone(), limit: BUDGET as u64 }));
            return link(written);
        }
        let Some(part) = Part::of(&document.blocks, anchor.as_ref()) else {
            self.said.push(Said::Problem(Problem::EmbedAnchorMissing { written: written.clone() }));
            return link(written);
        };
        self.budget -= 1;
        let mut carried = Carried { id, dropped: false, taken: &mut self.taken };
        let blocks = carried.blocks(part);
        if carried.dropped {
            self.said.push(Said::Notice(Announcement::EmbedFootnoteDropped { written }));
        }
        self.shown.push((id, anchor.clone()));
        let blocks = self.blocks(blocks);
        self.shown.pop();
        Block::Embed { target: LinkTarget::Page { id, anchor }, blocks }
    }
}

/// The part of a page an anchor names.
#[derive(Debug)]
struct Part;

impl Part {
    /// The blocks `anchor` names in `blocks`: all of them for no anchor; for a heading's anchor
    /// the heading and what follows up to the next heading of its level or above; for a block
    /// id, the innermost block holding it, or the block before when the id stands alone.
    fn of(blocks: &[Block], anchor: Option<&Anchor>) -> Option<Vec<Block>> {
        let Some(anchor) = anchor else { return Some(blocks.to_vec()) };
        let heading = blocks.iter().position(|block| matches!(block, Block::Heading { anchor: a, .. } if a == anchor));
        if let Some(at) = heading
            && let Some(Block::Heading { level, .. }) = blocks.get(at)
        {
            let rest = blocks.get(at + 1..).unwrap_or_default();
            let end = rest
                .iter()
                .position(|block| matches!(block, Block::Heading { level: next, .. } if next <= level))
                .map_or(blocks.len(), |end| at + 1 + end);
            return blocks.get(at..end).map(<[Block]>::to_vec);
        }
        Self::block(blocks, anchor).map(|block| vec![block])
    }

    /// The innermost block of `blocks` whose own inlines hold `anchor`; for a paragraph holding
    /// only it, the block before.
    fn block(blocks: &[Block], anchor: &Anchor) -> Option<Block> {
        for (at, block) in blocks.iter().enumerate() {
            let own = match block {
                Block::Paragraph(inlines) | Block::Heading { inlines, .. } => Some(inlines),
                _ => None,
            };
            if let Some(inlines) = own
                && inlines.iter().any(|inline| matches!(inline, Inline::Anchor(a) if a == anchor))
            {
                let alone = inlines.iter().all(|inline| match inline {
                    Inline::Anchor(_) => true,
                    Inline::Text(text) => text.trim().is_empty(),
                    _ => false,
                });
                return if alone {
                    at.checked_sub(1).and_then(|before| blocks.get(before)).cloned()
                } else {
                    Some(block.clone())
                };
            }
            if let Some(found) = Self::children(block).iter().find_map(|children| Self::block(children, anchor)) {
                return Some(found);
            }
        }
        None
    }

    /// The runs of blocks a block holds.
    fn children(block: &Block) -> Vec<&[Block]> {
        match block {
            Block::Quote(blocks)
            | Block::Callout { blocks, .. }
            | Block::Details { blocks, .. }
            | Block::Embed { blocks, .. } => {
                vec![blocks]
            }
            Block::List { items, .. } => items.iter().map(|item| item.blocks.as_slice()).collect(),
            Block::Definitions(definitions) => {
                definitions.iter().flat_map(|definition| definition.details.iter().map(Vec::as_slice)).collect()
            }
            _ => Vec::new(),
        }
    }
}

/// What an embedded part of the page `id` becomes on another page: its links to anchors point
/// at `id`, its footnote references and block ids are removed, as both stay on `id`.
#[derive(Debug)]
struct Carried<'a> {
    id: PageId,
    /// Whether a footnote reference was removed.
    dropped: bool,
    /// The anchors of the page it is shown on, to which each heading's anchor is added.
    taken: &'a mut BTreeSet<Anchor>,
}

impl Carried<'_> {
    /// `anchor`, or the first of `anchor-1`, `anchor-2` the page does not have, now taken.
    fn unique(&mut self, anchor: Anchor) -> Anchor {
        if self.taken.insert(anchor.clone()) {
            return anchor;
        }
        let renamed = (1..=u64::from(u32::MAX))
            .filter_map(|count| Slug::parse(format!("{anchor}-{count}")).ok())
            .map(Anchor::new)
            .find(|candidate| self.taken.insert(candidate.clone()));
        // a slug with `-` and digits after it is a slug, so one is always found
        renamed.unwrap_or(anchor)
    }
}

impl Fold for Carried<'_> {
    fn block(&mut self, block: Block) -> Block {
        match block {
            Block::Heading { level, anchor, inlines } => {
                let anchor = self.unique(anchor);
                Block::Heading { level, anchor, inlines: self.inlines(inlines) }
            }
            Block::Embed { target: LinkTarget::Anchor(anchor), blocks } => {
                Block::Embed { target: LinkTarget::Page { id: self.id, anchor: Some(anchor) }, blocks }
            }
            block => block.fold(self),
        }
    }

    fn inlines(&mut self, inlines: Vec<Inline>) -> Vec<Inline> {
        let mut kept = Vec::with_capacity(inlines.len());
        for inline in inlines {
            match inline {
                Inline::Anchor(_) => {}
                Inline::Footnote(_) => self.dropped = true,
                inline => kept.push(self.inline(inline)),
            }
        }
        kept
    }

    fn link(&mut self, target: LinkTarget, children: Vec<Inline>) -> Inline {
        let target = match target {
            LinkTarget::Anchor(anchor) => LinkTarget::Page { id: self.id, anchor: Some(anchor) },
            target => target,
        };
        Inline::Link { target, children: self.inlines(children) }
    }
}
