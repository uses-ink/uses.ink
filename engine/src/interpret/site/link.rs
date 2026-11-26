//! Link resolution. Formats leave links to files as written; once every page is decided, each
//! published page's links and images are pointed at pages, anchors and assets. A link that
//! names nothing, an unpublished file, a plain file, or several files stays plain text and says
//! why: nothing from an unpublished file reaches a published page.

mod target;

use std::collections::BTreeMap;

use model::{
    Anchor, Block, ContentPath, Diagnostic, ExternalUrl, ImageSource, Inline, LinkTarget, PageId, Problem, Slug,
};

use target::Found;
pub(crate) use target::Targets;

use crate::interpret::file::read::Source;
use crate::interpret::record::Record;

/// Resolves every link of every published page, in its body and its footnotes. Returns each plain file a published page shows
/// as an image, with the first page that does.
pub(crate) fn resolve(
    sources: &mut [Source],
    targets: &Targets<'_>,
    record: &mut Record,
) -> BTreeMap<ContentPath, ContentPath> {
    let mut used = BTreeMap::new();
    for source in sources.iter_mut().filter(|source| targets.published.contains(&source.path)) {
        let Source { path, read, .. } = source;
        let mut problems = Vec::new();
        let mut page = Page { from: path, targets, problems: &mut problems, used: &mut used };
        let document = &mut read.document;
        let footnotes = document.footnotes.iter_mut().map(|footnote| &mut footnote.blocks);
        for part in std::iter::once(&mut document.blocks).chain(footnotes) {
            blocks(part, &mut |inline| page.inline(inline));
        }
        for problem in problems {
            record.diagnose(Diagnostic::new(problem).on(source.subject()));
        }
    }
    used
}

/// One page's links being resolved.
struct Page<'a, 'b> {
    from: &'a ContentPath,
    targets: &'a Targets<'b>,
    problems: &'a mut Vec<Problem>,
    used: &'a mut BTreeMap<ContentPath, ContentPath>,
}

impl Page<'_, '_> {
    fn inline(&mut self, inline: &mut Inline) {
        match inline {
            Inline::Link { target, .. } => {
                if let LinkTarget::Unresolved { written } = target
                    && let Some(resolved) = self.link(written)
                {
                    *target = resolved;
                }
            }
            Inline::Image { source, .. } => {
                if let ImageSource::Unresolved { written } = source
                    && let Some(resolved) = self.image(written)
                {
                    *source = resolved;
                }
            }
            _ => {}
        }
    }

    fn link(&mut self, written: &str) -> Option<LinkTarget> {
        let (path, anchor) = written.split_once('#').map_or((written, None), |(path, anchor)| (path, Some(anchor)));
        let anchor = anchor.and_then(|anchor| Slug::from_text(anchor).ok()).map(Anchor::new);
        if path.is_empty() {
            return self.missing(written, anchor.map(LinkTarget::Anchor));
        }
        if has_scheme(path) {
            return self.missing(written, ExternalUrl::parse(written).ok().map(LinkTarget::External));
        }
        match self.targets.find(self.from, path) {
            Found::Page(page) => Some(LinkTarget::Page { id: PageId::of(&page), anchor }),
            Found::File(target) => self.refuse(Problem::LinkTargetAsset { written: written.to_owned(), target }),
            found => self.unfound(written, found),
        }
    }

    fn image(&mut self, written: &str) -> Option<ImageSource> {
        if has_scheme(written) {
            return self.missing(written, ExternalUrl::parse(written).ok().map(ImageSource::External));
        }
        let path = written.split_once('#').map_or(written, |(path, _)| path);
        match self.targets.find(self.from, path) {
            Found::File(file) => {
                self.used.entry(file.clone()).or_insert_with(|| self.from.clone());
                Some(ImageSource::Asset(file))
            }
            Found::Page(_) => self.missing(written, None),
            found => self.unfound(written, found),
        }
    }

    /// The problem of a link that names no file it can show, or several.
    fn unfound<T>(&mut self, written: &str, found: Found) -> Option<T> {
        let written = written.to_owned();
        match found {
            Found::Unpublished(target) => self.refuse(Problem::LinkTargetUnpublished { written, target }),
            Found::Ambiguous(candidates) => self.refuse(Problem::LinkTargetAmbiguous { written, candidates }),
            Found::Missing | Found::Page(_) | Found::File(_) => self.missing(&written, None),
        }
    }

    /// `resolved`, or `link.target.missing` when there is none.
    fn missing<T>(&mut self, written: &str, resolved: Option<T>) -> Option<T> {
        if resolved.is_none() {
            self.problems.push(Problem::LinkTargetMissing { written: written.to_owned() });
        }
        resolved
    }

    fn refuse<T>(&mut self, problem: Problem) -> Option<T> {
        self.problems.push(problem);
        None
    }
}

/// Whether `written` starts with a URL scheme (`https:`, `mailto:`, `javascript:`), so it is
/// never a path.
fn has_scheme(written: &str) -> bool {
    written.split_once(':').is_some_and(|(scheme, _)| {
        let mut chars = scheme.chars();
        chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

/// Every inline of `blocks`, depth first, in document order.
fn blocks(blocks: &mut [Block], visit: &mut impl FnMut(&mut Inline)) {
    for block in blocks {
        match block {
            Block::Heading { inlines, .. } | Block::Paragraph(inlines) => self::inlines(inlines, visit),
            Block::List { items, .. } => items.iter_mut().for_each(|item| self::blocks(&mut item.blocks, visit)),
            Block::Quote(children) => self::blocks(children, visit),
            Block::Definitions(definitions) => {
                for definition in definitions {
                    self::inlines(&mut definition.term, visit);
                    definition.details.iter_mut().for_each(|detail| self::blocks(detail, visit));
                }
            }
            Block::Details { summary, blocks: children } => {
                self::inlines(summary, visit);
                self::blocks(children, visit);
            }
            Block::Table { head, rows, .. } => {
                head.iter_mut().chain(rows.iter_mut().flatten()).for_each(|cell| self::inlines(cell, visit));
            }
            Block::Callout { title, blocks: children, .. } => {
                if let Some(title) = title {
                    self::inlines(title, visit);
                }
                self::blocks(children, visit);
            }
            _ => {}
        }
    }
}

fn inlines(inlines: &mut [Inline], visit: &mut impl FnMut(&mut Inline)) {
    for inline in inlines {
        visit(inline);
        match inline {
            Inline::Emphasis(children)
            | Inline::Strong(children)
            | Inline::Strike(children)
            | Inline::Highlight(children)
            | Inline::Superscript(children)
            | Inline::Subscript(children)
            | Inline::Link { children, .. } => self::inlines(children, visit),
            _ => {}
        }
    }
}
