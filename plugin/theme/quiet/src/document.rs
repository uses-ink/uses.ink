//! A page's [`Document`] as semantic HTML: one [`Visit`], [`Writer`], that writes each node
//! through `unspliced` as the walk reaches it. An element with children is a shell holding
//! [`HERE`]: the writer writes the shell up to it, visits the children, then writes the rest.
//!
//! What it writes:
//!
//! - Headings get their anchor as `id` and a `#` link to it.
//! - Task items are `<input type="checkbox" disabled>`: shown, never changed, no script.
//! - Links to pages go through the site's routes; a link that resolved to nothing is its text.
//! - A callout a reader can fold is a `<details>`, open at first when written with `+`; a block
//!   id is an empty `<span>` with its anchor as `id`; an embed is the blocks it shows in a
//!   `<div>` of the embed class.
//! - Images of the snapshot are shown as the engine prepared them ([`picture`]): AVIF and a
//!   fallback at each width, with their size and a placeholder; one that resolved to nothing is
//!   its alt text.
//! - Footnotes are numbered in order of first reference and listed at the end, each with a link
//!   back to every reference ([`note`]).
//! - Code is highlighted by the host when a grammar knows its language
//!   (`code.language.unknown` otherwise), each run a class of the code stylesheet ([`code`]);
//!   math is `MathML` (`math.syntax.invalid` when it does not parse, shown as written).

mod code;
mod math;
mod note;
mod picture;

use std::collections::BTreeMap;
use std::fmt;

use engine::Record;
use model::highlight::Highlight;
use model::{
    Alignment, Block, CalloutKind, Document, Folding, Footnote, FootnoteName, ImageSource, Inline, Item, LinkTarget,
    Page, Site, Subject, Task, Visit,
};
use unspliced::html::{attr, tag};
use unspliced::{Class, HERE, Halt, Id, Marks, Render, Sink, Split, Url, html};

use math::Placement;
use picture::Shown;

use crate::style::quiet;

/// Why a page was not written whole. Either is a bug in the theme, never in the page.
#[derive(Debug)]
pub(crate) enum Unwritten {
    /// The sink refused a write.
    Halt(Halt),
    /// A shell holds no [`HERE`], or several.
    Marks(Marks),
}

impl fmt::Display for Unwritten {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Halt(halt) => write!(out, "the page could not be written: {halt}"),
            Self::Marks(marks) => write!(out, "a shell of the theme is broken: {marks}"),
        }
    }
}

impl std::error::Error for Unwritten {}

/// Whether a list has blank lines between its items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spacing {
    Tight,
    Loose,
}

/// What a writer reads: the page, the site its links resolve against, and the highlighter its
/// code goes through.
#[derive(Debug)]
pub(crate) struct Source<'a, H> {
    pub(crate) page: &'a Page,
    pub(crate) site: &'a Site,
    pub(crate) highlight: &'a H,
}

impl<H> Clone for Source<'_, H> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<H> Copy for Source<'_, H> {}

/// The walk over one page's document, writing into `out` and recording what it finds worth
/// saying into `record`. After the first write `out` refuses, it writes nothing more.
pub(crate) struct Writer<'a, H, R: ?Sized> {
    source: Source<'a, H>,
    out: &'a mut dyn Sink,
    record: &'a mut R,
    /// The first failure; nothing is written after one.
    failed: Option<Unwritten>,
    /// The list being written, for its items.
    spacing: Spacing,
    /// How many references to each footnote are written so far.
    references: BTreeMap<FootnoteName, usize>,
    /// Each footnote's number, from 1 in the order listed: looked up once per reference, so
    /// held by name rather than searched for.
    numbers: BTreeMap<&'a FootnoteName, usize>,
}

impl<H, R: ?Sized> fmt::Debug for Writer<'_, H, R> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("Writer")
            .field("page", &self.source.page.file)
            .field("failed", &self.failed)
            .finish_non_exhaustive()
    }
}

impl<'a, H: Highlight, R: Record + ?Sized> Writer<'a, H, R> {
    /// A writer of the source's page.
    pub(crate) fn new(source: Source<'a, H>, out: &'a mut dyn Sink, record: &'a mut R) -> Self {
        let references = BTreeMap::new();
        let mut numbers = BTreeMap::new();
        for (at, footnote) in source.page.body.footnotes.iter().enumerate() {
            // the first of a name, as `Document::footnote_number` has it
            numbers.entry(&footnote.name).or_insert(at + 1);
        }
        Self { source, out, record, failed: None, spacing: Spacing::Loose, references, numbers }
    }

    /// The page's document, written whole, or why it was not.
    pub(crate) fn write(mut self) -> Result<(), Unwritten> {
        let page = self.source.page;
        self.document(&page.body);
        self.failed.map_or(Ok(()), Err)
    }

    /// Writes `markup`.
    fn put(&mut self, markup: impl Render) {
        if self.failed.is_none()
            && let Err(halt) = markup.render(self.out)
        {
            self.failed = Some(Unwritten::Halt(halt));
        }
    }

    /// Writes `shell` around what `inner` writes where the shell holds [`HERE`].
    fn wrap(&mut self, shell: impl Render, inner: impl FnOnce(&mut Self)) {
        if self.failed.is_some() {
            return;
        }
        let split = match Split::new(&shell) {
            Ok(split) => split,
            Err(marks) => {
                self.failed = Some(Unwritten::Marks(marks));
                return;
            }
        };
        self.put(Half::Head(&split));
        inner(self);
        self.put(Half::Tail(&split));
    }

    /// What a diagnostic or notice of this page is about.
    fn subject(&self) -> Subject {
        Subject::File(self.source.page.file.clone())
    }

    fn table(&mut self, alignments: &[Alignment], head: &[Vec<Inline>], rows: &[Vec<Vec<Inline>>]) {
        let class = |column: usize| match alignments.get(column) {
            Some(Alignment::Left) => Some(quiet::LEFT),
            Some(Alignment::Center) => Some(quiet::CENTER),
            Some(Alignment::Right) => Some(quiet::RIGHT),
            _ => None::<Class>,
        };
        self.wrap(html! { table { (HERE) } }, |table| {
            table.wrap(html! { thead { tr { (HERE) } } }, |head_row| {
                for (column, cell) in head.iter().enumerate() {
                    head_row.wrap(html! { th class=(class(column)) { (HERE) } }, |th| th.inlines(cell));
                }
            });
            table.wrap(html! { tbody { (HERE) } }, |body| {
                for row in rows {
                    body.wrap(html! { tr { (HERE) } }, |tr| {
                        for (column, cell) in row.iter().enumerate() {
                            tr.wrap(html! { td class=(class(column)) { (HERE) } }, |td| td.inlines(cell));
                        }
                    });
                }
            });
        });
    }

    /// A callout; one a reader can fold is a `<details>`, open at first for `+`.
    fn callout(&mut self, callout: &Block) {
        let Block::Callout { kind, title, blocks, folding } = callout else { return };
        let (class, label) = match kind {
            CalloutKind::Tip => (Some(quiet::TIP), "Tip"),
            CalloutKind::Important => (None, "Important"),
            CalloutKind::Warning => (Some(quiet::WARNING), "Warning"),
            CalloutKind::Caution => (Some(quiet::CAUTION), "Caution"),
            CalloutKind::Other(slug) => (None, slug.as_str()),
            // a note, and kinds this theme does not know yet
            _ => (None, "Note"),
        };
        let heading = |line: &mut Self| match title {
            Some(title) => line.inlines(title),
            None => line.put(label),
        };
        match folding {
            Folding::Open | Folding::Closed => {
                let open = *folding == Folding::Open;
                self.wrap(html! { details.quiet::CALLOUT class=(class) open=(open) { (HERE) } }, |details| {
                    details.wrap(html! { summary.quiet::LABEL { (HERE) } }, heading);
                    details.blocks(blocks);
                });
            }
            // fixed, and foldings this theme does not know yet
            _ => self.wrap(html! { aside.quiet::CALLOUT class=(class) { (HERE) } }, |aside| {
                aside.wrap(html! { p.quiet::LABEL { (HERE) } }, heading);
                aside.blocks(blocks);
            }),
        }
    }
}

impl<H: Highlight, R: Record + ?Sized> Visit for Writer<'_, H, R> {
    /// The blocks, then the footnotes in a section of their own.
    fn document(&mut self, document: &Document) {
        self.blocks(&document.blocks);
        if !document.footnotes.is_empty() {
            let notes = html! { section.quiet::NOTES aria-label="Footnotes" { ol { (HERE) } } };
            self.wrap(notes, |notes| document.footnotes.iter().for_each(|footnote| notes.footnote(footnote)));
        }
    }

    fn footnote(&mut self, footnote: &Footnote) {
        self.note(footnote);
    }

    fn block(&mut self, block: &Block) {
        match block {
            Block::Heading { level, anchor, .. } => {
                let level = tag::Level::new(level.get()).unwrap_or(tag::Level::Six);
                let content = (HERE, Permalink(anchor.as_str()));
                match Id::checked(anchor.as_str()) {
                    Ok(id) => self.wrap(level.tag().attrs([attr::global::id.is(id)]).with(content), |h| block.walk(h)),
                    // An anchor is a slug: never empty, never holding whitespace.
                    Err(_) => self.wrap(level.tag().with(content), |h| block.walk(h)),
                }
            }
            Block::Paragraph(_) => self.wrap(html! { p { (HERE) } }, |p| block.walk(p)),
            Block::List { ordered, tight, .. } => {
                let outer = std::mem::replace(&mut self.spacing, if *tight { Spacing::Tight } else { Spacing::Loose });
                match *ordered {
                    None => self.wrap(html! { ul { (HERE) } }, |list| block.walk(list)),
                    Some(1) => self.wrap(html! { ol { (HERE) } }, |list| block.walk(list)),
                    Some(start) => self.wrap(html! { ol start=(start) { (HERE) } }, |list| block.walk(list)),
                }
                self.spacing = outer;
            }
            Block::Quote(_) => self.wrap(html! { blockquote { (HERE) } }, |quote| block.walk(quote)),
            Block::Code { language, text } => self.code(language.as_deref(), text),
            Block::Rule => self.put(html! { hr; }),
            Block::Table { alignments, head, rows } => self.table(alignments, head, rows),
            Block::Callout { .. } => self.callout(block),
            Block::Math(tex) => self.wrap(html! { div.quiet::MATH { (HERE) } }, |div| div.math(tex, Placement::Block)),
            Block::Definitions(_) => self.wrap(html! { dl { (HERE) } }, |list| block.walk(list)),
            Block::Details { summary, blocks } => self.wrap(html! { details { (HERE) } }, |details| {
                details.wrap(html! { summary { (HERE) } }, |line| line.inlines(summary));
                details.blocks(blocks);
            }),
            Block::Embed { .. } => self.wrap(html! { div.quiet::EMBED { (HERE) } }, |div| block.walk(div)),
            // kinds this theme does not know yet: their children, as text
            _ => block.walk(self),
        }
    }

    /// A list item's blocks; in a tight list a paragraph is its text alone, as `CommonMark` has it.
    fn item(&mut self, item: &Item) {
        let content = |li: &mut Self| {
            for block in &item.blocks {
                match (li.spacing, block) {
                    (Spacing::Tight, Block::Paragraph(inlines)) => li.inlines(inlines),
                    _ => li.block(block),
                }
            }
        };
        match item.task {
            None => self.wrap(html! { li { (HERE) } }, content),
            Some(task) => {
                let done = task == Task::Done;
                self.wrap(html! { li.quiet::TASK { input type="checkbox" disabled checked=(done); (HERE) } }, content);
            }
        }
    }

    fn definition(&mut self, definition: &model::Definition) {
        self.wrap(html! { dt { (HERE) } }, |dt| dt.inlines(&definition.term));
        for details in &definition.details {
            self.wrap(html! { dd { (HERE) } }, |dd| dd.blocks(details));
        }
    }

    fn inline(&mut self, inline: &Inline) {
        match inline {
            Inline::Emphasis(_) => self.wrap(html! { em { (HERE) } }, |em| inline.walk(em)),
            Inline::Strong(_) => self.wrap(html! { strong { (HERE) } }, |strong| inline.walk(strong)),
            Inline::Strike(_) => self.wrap(html! { s { (HERE) } }, |s| inline.walk(s)),
            Inline::Highlight(_) => self.wrap(html! { mark { (HERE) } }, |mark| inline.walk(mark)),
            Inline::Superscript(_) => self.wrap(html! { sup { (HERE) } }, |sup| inline.walk(sup)),
            Inline::Subscript(_) => self.wrap(html! { sub { (HERE) } }, |sub| inline.walk(sub)),
            Inline::Code(text) => self.put(html! { code { (text) } }),
            Inline::Keyboard(keys) => self.put(html! { kbd { (keys) } }),
            Inline::Break => self.put(html! { br; }),
            Inline::SoftBreak => self.put("\n"),
            Inline::Math(tex) => self.math(tex, Placement::Inline),
            Inline::Footnote(name) => self.reference(name),
            // An anchor is a slug: never empty, never holding whitespace, so always an id.
            Inline::Anchor(anchor) => {
                if let Ok(id) = Id::checked(anchor.as_str()) {
                    self.put(html! { span id=(id) {} });
                }
            }
            // text, links and images through their own methods; kinds this theme does not know
            // yet through their children
            _ => inline.walk(self),
        }
    }

    fn link(&mut self, target: &LinkTarget, children: &[Inline]) {
        let href = match target {
            LinkTarget::Page { id, anchor } => self.source.site.page(*id).map(|page| match anchor {
                Some(anchor) => format!("{}#{anchor}", page.route),
                None => page.route.to_string(),
            }),
            LinkTarget::Anchor(anchor) => Some(format!("#{anchor}")),
            LinkTarget::External(url) => Some(url.to_string()),
            _ => None,
        };
        match href.as_deref().map(Url::checked) {
            Some(Ok(url)) => self.wrap(html! { a href=(url) { (HERE) } }, |a| a.inlines(children)),
            // Unresolved, or a scheme no reader page links to: the text alone.
            _ => self.inlines(children),
        }
    }

    fn image(&mut self, source: &ImageSource, alt: &str) {
        if let ImageSource::Asset(path) = source
            && let Some(asset) = self.source.site.assets().iter().find(|asset| &asset.file == path)
            && let Some(picture) = &asset.picture
        {
            return self.put(Shown { asset, picture, alt });
        }
        let src = match source {
            ImageSource::Asset(path) => {
                self.source.site.assets().iter().find(|asset| &asset.file == path).map(|asset| asset.route.to_string())
            }
            ImageSource::External(url) => Some(url.to_string()),
            _ => None,
        };
        match src.as_deref().map(Url::checked) {
            Some(Ok(url)) => self.put(html! { img src=(url) alt=(alt); }),
            _ => self.put(alt),
        }
    }

    fn text(&mut self, text: &str) {
        self.put(text);
    }
}

/// The `#` link a heading carries to itself.
#[derive(Clone, Copy, Debug)]
struct Permalink<'a>(&'a str);

impl Render for Permalink<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let Ok(id) = Id::checked(self.0) else { return Ok(()) };
        html! { a.quiet::ANCHOR href=(Url::to(id)) aria-label="Link to this section" { "#" } }.render(out)
    }
}

/// One side of a split shell, as something to write.
#[derive(Clone, Copy, Debug)]
enum Half<'s, 'p, P: ?Sized> {
    /// Up to the mark.
    Head(&'s Split<'p, P>),
    /// From the mark.
    Tail(&'s Split<'p, P>),
}

impl<P: Render + ?Sized> Render for Half<'_, '_, P> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        match self {
            Self::Head(split) => split.head(out),
            Self::Tail(split) => split.tail(out),
        }
    }
}
