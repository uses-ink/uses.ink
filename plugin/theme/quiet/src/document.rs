//! A page's [`Document`](model::Document) as semantic HTML, apart from the page around it, so a
//! second theme can reuse it. The whole walk is one type, [`Walk`], which becomes an
//! implementation of a shared document visitor once there is one.
//!
//! What it writes:
//!
//! - Headings get their anchor as `id` and a `#` link to it.
//! - Task items are `<input type="checkbox" disabled>`: shown, never changed, no script.
//! - Links to pages go through the site's routes; a link that resolved to nothing is its text.
//! - Images of the snapshot go through their asset route; one that resolved to nothing is its
//!   alt text.
//! - Footnotes are numbered in order of first reference and listed at the end, each with a link
//!   back to every reference.
//! - Code is highlighted when giallo knows its language (`code.language.unknown` otherwise) and
//!   math is `MathML` (`math.syntax.invalid` when it does not parse, shown as written).

mod code;
mod math;

use std::cell::RefCell;
use std::collections::BTreeMap;

use model::{
    Alignment, Block, CalloutKind, Diagnostic, Footnote, FootnoteName, ImageSource, Inline, Item, LinkTarget, Notice,
    Page, Problem, Site, Subject, Task,
};
use unspliced::html::{attr, tag};
use unspliced::{Class, Halt, Id, Lazy, Raw, Render, Sink, Url, html};

use code::Highlighted;
pub(crate) use code::Highlighter;
use math::{Display, Math};

use crate::Said;
use crate::style::quiet;

/// A page's body and its footnotes, ready to render.
#[derive(Debug)]
pub struct Body<'a> {
    walk: Walk<'a>,
}

impl<'a> Body<'a> {
    /// The body of `page`, whose links resolve against `site`. What rendering finds worth saying
    /// is added to `said`.
    pub fn new(page: &'a Page, site: &'a Site, said: &'a RefCell<Said>) -> Self {
        Self { walk: Walk { site, page, said, references: RefCell::new(BTreeMap::new()) } }
    }
}

impl Render for Body<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        self.walk.blocks(&self.walk.page.body.blocks, out)?;
        self.walk.notes(out)
    }
}

/// The walk over one document.
#[derive(Debug)]
struct Walk<'a> {
    site: &'a Site,
    page: &'a Page,
    said: &'a RefCell<Said>,
    /// How many references to each footnote are written so far.
    references: RefCell<BTreeMap<FootnoteName, usize>>,
}

/// Whether a list has blank lines between its items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Spacing {
    Tight,
    Loose,
}

/// The ids of a footnote and of its references.
#[derive(Clone, Copy, Debug)]
struct Note<'a>(&'a FootnoteName);

impl Note<'_> {
    /// `fn:<name>`. A heading anchor is a slug, which never holds `:`, so the two never meet.
    fn id(self) -> String {
        format!("fn:{}", self.0)
    }

    /// `ref:<name>` for the first reference, `ref:<name>:<n>` for the n-th.
    fn reference(self, nth: usize) -> String {
        if nth == 1 { format!("ref:{}", self.0) } else { format!("ref:{}:{nth}", self.0) }
    }
}

impl Walk<'_> {
    /// `blocks` as something to put in a hole.
    fn flow<'b>(&'b self, blocks: &'b [Block]) -> impl Render + 'b {
        Lazy(move |out: &mut dyn Sink| self.blocks(blocks, out))
    }

    /// `inlines` as something to put in a hole.
    fn run<'b>(&'b self, inlines: &'b [Inline]) -> impl Render + 'b {
        Lazy(move |out: &mut dyn Sink| self.inlines(inlines, out))
    }

    fn blocks(&self, blocks: &[Block], out: &mut dyn Sink) -> Result<(), Halt> {
        blocks.iter().try_for_each(|block| self.block(block, out))
    }

    fn inlines(&self, inlines: &[Inline], out: &mut dyn Sink) -> Result<(), Halt> {
        inlines.iter().try_for_each(|inline| self.inline(inline, out))
    }

    fn block(&self, block: &Block, out: &mut dyn Sink) -> Result<(), Halt> {
        match block {
            Block::Heading { level, anchor, inlines } => {
                let level = tag::Level::new(level.get()).unwrap_or(tag::Level::Six);
                let content = (self.run(inlines), Lazy(|out: &mut dyn Sink| Self::anchor(anchor.as_str(), out)));
                match Id::checked(anchor.as_str()) {
                    Ok(id) => level.tag().attrs([attr::global::id.is(id)]).with(content).render(out),
                    // An anchor is a slug: never empty, never holding whitespace.
                    Err(_) => level.tag().with(content).render(out),
                }
            }
            Block::Paragraph(inlines) => html! { p { (self.run(inlines)) } }.render(out),
            Block::List { ordered, items, tight } => {
                let spacing = if *tight { Spacing::Tight } else { Spacing::Loose };
                self.list(*ordered, items, spacing, out)
            }
            Block::Quote(blocks) => html! { blockquote { (self.flow(blocks)) } }.render(out),
            Block::Code { language, text } => self.code(language.as_deref(), text, out),
            Block::Rule => html! { hr; }.render(out),
            Block::Table { alignments, head, rows } => self.table(alignments, head, rows, out),
            Block::Callout { kind, title, blocks } => self.callout(kind, title.as_deref(), blocks, out),
            Block::Math(tex) => {
                let math = self.math(tex, Display::Block);
                html! { div.quiet::MATH { (math) } }.render(out)
            }
            Block::Definitions(definitions) => html! {
                dl {
                    @for definition in definitions {
                        dt { (self.run(&definition.term)) }
                        @for details in &definition.details { dd { (self.flow(details)) } }
                    }
                }
            }
            .render(out),
            Block::Details { summary, blocks } => {
                html! { details { summary { (self.run(summary)) } (self.flow(blocks)) } }.render(out)
            }
            _ => Ok(()),
        }
    }

    fn anchor(anchor: &str, out: &mut dyn Sink) -> Result<(), Halt> {
        let Ok(id) = Id::checked(anchor) else { return Ok(()) };
        html! { a.quiet::ANCHOR href=(Url::to(id)) aria-label="Link to this section" { "#" } }.render(out)
    }

    fn list(&self, ordered: Option<u64>, items: &[Item], spacing: Spacing, out: &mut dyn Sink) -> Result<(), Halt> {
        let items = Lazy(move |out: &mut dyn Sink| {
            items.iter().try_for_each(|item| {
                let content = Lazy(|out: &mut dyn Sink| self.item(&item.blocks, spacing, out));
                match item.task {
                    None => html! { li { (content) } }.render(out),
                    Some(task) => {
                        let done = task == Task::Done;
                        html! { li.quiet::TASK { input type="checkbox" disabled checked=(done); (content) } }
                            .render(out)
                    }
                }
            })
        });
        match ordered {
            None => html! { ul { (items) } }.render(out),
            Some(1) => html! { ol { (items) } }.render(out),
            Some(start) => html! { ol start=(start) { (items) } }.render(out),
        }
    }

    /// A list item's blocks; in a tight list a paragraph is its text alone, as `CommonMark` has it.
    fn item(&self, blocks: &[Block], spacing: Spacing, out: &mut dyn Sink) -> Result<(), Halt> {
        blocks.iter().try_for_each(|block| match (spacing, block) {
            (Spacing::Tight, Block::Paragraph(inlines)) => self.inlines(inlines, out),
            _ => self.block(block, out),
        })
    }

    fn code(&self, language: Option<&str>, text: &str, out: &mut dyn Sink) -> Result<(), Halt> {
        let plain = |out: &mut dyn Sink| html! { pre { code { (text) } } }.render(out);
        let Some(language) = language else { return plain(out) };
        let highlighted = match Highlighter::load() {
            Ok(highlighter) => highlighter.highlight(language, text),
            Err(reason) => Highlighted::Broken(reason.to_owned()),
        };
        match highlighted {
            // giallo escapes the code's text and writes only spans with its own classes.
            Highlighted::Html(html) => Raw::trusted(&html).render(out),
            Highlighted::Unknown => {
                let notice = model::Announcement::CodeLanguageUnknown { language: language.to_owned() };
                self.said.borrow_mut().notices.push(Notice::new(notice).on(self.subject()));
                plain(out)
            }
            Highlighted::Broken(reason) => {
                let problem = Problem::RenderPageFailed { reason };
                self.said.borrow_mut().diagnostics.push(Diagnostic::new(problem).on(self.subject()));
                plain(out)
            }
        }
    }

    fn table(
        &self,
        alignments: &[Alignment],
        head: &[Vec<Inline>],
        rows: &[Vec<Vec<Inline>>],
        out: &mut dyn Sink,
    ) -> Result<(), Halt> {
        let class = |column: usize| match alignments.get(column) {
            Some(Alignment::Left) => Some(quiet::LEFT),
            Some(Alignment::Center) => Some(quiet::CENTER),
            Some(Alignment::Right) => Some(quiet::RIGHT),
            _ => None::<Class>,
        };
        html! {
            table {
                thead { tr { @for (column, cell) in head.iter().enumerate() { th class=(class(column)) { (self.run(cell)) } } } }
                tbody {
                    @for row in rows {
                        tr { @for (column, cell) in row.iter().enumerate() { td class=(class(column)) { (self.run(cell)) } } }
                    }
                }
            }
        }
        .render(out)
    }

    fn callout(
        &self,
        kind: &CalloutKind,
        title: Option<&[Inline]>,
        blocks: &[Block],
        out: &mut dyn Sink,
    ) -> Result<(), Halt> {
        let (class, label) = match kind {
            CalloutKind::Tip => (Some(quiet::TIP), "Tip"),
            CalloutKind::Important => (None, "Important"),
            CalloutKind::Warning => (Some(quiet::WARNING), "Warning"),
            CalloutKind::Caution => (Some(quiet::CAUTION), "Caution"),
            CalloutKind::Other(slug) => (None, slug.as_str()),
            // a note, and kinds this theme does not know yet
            _ => (None, "Note"),
        };
        let title = Lazy(move |out: &mut dyn Sink| match title {
            Some(title) => self.inlines(title, out),
            None => label.render(out),
        });
        html! { aside.quiet::CALLOUT class=(class) { p.quiet::LABEL { (title) } (self.flow(blocks)) } }.render(out)
    }

    fn inline(&self, inline: &Inline, out: &mut dyn Sink) -> Result<(), Halt> {
        match inline {
            Inline::Text(text) => text.render(out),
            Inline::Emphasis(inlines) => html! { em { (self.run(inlines)) } }.render(out),
            Inline::Strong(inlines) => html! { strong { (self.run(inlines)) } }.render(out),
            Inline::Strike(inlines) => html! { s { (self.run(inlines)) } }.render(out),
            Inline::Code(text) => html! { code { (text) } }.render(out),
            Inline::Link { target, children } => self.link(target, children, out),
            Inline::Image { source, alt } => self.image(source, alt, out),
            Inline::Break => html! { br; }.render(out),
            Inline::SoftBreak => "\n".render(out),
            Inline::Math(tex) => self.math(tex, Display::Inline).render(out),
            Inline::Footnote(name) => self.reference(name, out),
            Inline::Highlight(inlines) => html! { mark { (self.run(inlines)) } }.render(out),
            Inline::Superscript(inlines) => html! { sup { (self.run(inlines)) } }.render(out),
            Inline::Subscript(inlines) => html! { sub { (self.run(inlines)) } }.render(out),
            Inline::Keyboard(keys) => html! { kbd { (keys) } }.render(out),
            _ => Ok(()),
        }
    }

    fn link(&self, target: &LinkTarget, children: &[Inline], out: &mut dyn Sink) -> Result<(), Halt> {
        let href = match target {
            LinkTarget::Page { id, anchor } => self.site.page(*id).map(|page| match anchor {
                Some(anchor) => format!("{}#{anchor}", page.route),
                None => page.route.to_string(),
            }),
            LinkTarget::Anchor(anchor) => Some(format!("#{anchor}")),
            LinkTarget::External(url) => Some(url.to_string()),
            _ => None,
        };
        match href.as_deref().map(Url::checked) {
            Some(Ok(url)) => html! { a href=(url) { (self.run(children)) } }.render(out),
            // Unresolved, or a scheme no reader page links to: the text alone.
            _ => self.inlines(children, out),
        }
    }

    fn image(&self, source: &ImageSource, alt: &str, out: &mut dyn Sink) -> Result<(), Halt> {
        let src = match source {
            ImageSource::Asset(path) => {
                self.site.assets().iter().find(|asset| &asset.file == path).map(|asset| asset.route.to_string())
            }
            ImageSource::External(url) => Some(url.to_string()),
            _ => None,
        };
        match src.as_deref().map(Url::checked) {
            Some(Ok(url)) => html! { img src=(url) alt=(alt); }.render(out),
            _ => alt.render(out),
        }
    }

    /// Math as `MathML`, or as written when it does not parse.
    fn math<'b>(&'b self, tex: &'b str, display: Display) -> impl Render + 'b {
        Lazy(move |out: &mut dyn Sink| match Math::load().and_then(|math| math.mathml(tex, display)) {
            // math-core escapes every character it writes.
            Ok(mathml) => Raw::trusted(&mathml).render(out),
            Err(reason) => {
                let problem = Problem::MathSyntaxInvalid { tex: tex.to_owned(), reason };
                self.said.borrow_mut().diagnostics.push(Diagnostic::new(problem).on(self.subject()));
                html! { code.quiet::UNPARSED { (tex) } }.render(out)
            }
        })
    }

    fn reference(&self, name: &FootnoteName, out: &mut dyn Sink) -> Result<(), Halt> {
        let Some(number) = self.page.body.footnote_number(name) else { return name.to_string().render(out) };
        let nth = {
            let mut references = self.references.borrow_mut();
            let count = references.entry(name.clone()).or_insert(0);
            *count += 1;
            *count
        };
        let (note, reference) = (Note(name).id(), Note(name).reference(nth));
        match (Id::checked(&note), Id::checked(&reference)) {
            (Ok(note), Ok(reference)) => {
                html! { sup { a id=(reference) href=(Url::to(note)) { (number) } } }.render(out)
            }
            _ => html! { sup { (number) } }.render(out),
        }
    }

    /// The footnotes, numbered, each with links back to its references.
    fn notes(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let footnotes = &self.page.body.footnotes;
        if footnotes.is_empty() {
            return Ok(());
        }
        html! {
            section.quiet::NOTES aria-label="Footnotes" {
                ol { @for footnote in footnotes { (Lazy(|out: &mut dyn Sink| self.note(footnote, out))) } }
            }
        }
        .render(out)
    }

    fn note(&self, footnote: &Footnote, out: &mut dyn Sink) -> Result<(), Halt> {
        let id = Note(&footnote.name).id();
        let count = self.references.borrow().get(&footnote.name).copied().unwrap_or(0);
        let backs: Vec<String> = (1..=count).map(|nth| Note(&footnote.name).reference(nth)).collect();
        let back = Lazy(|out: &mut dyn Sink| {
            backs.iter().try_for_each(|back| match Id::checked(back) {
                Ok(back) => html! { a.quiet::BACK href=(Url::to(back)) aria-label="Back to the text" { "\u{21a9}" } }
                    .render(out),
                Err(_) => Ok(()),
            })
        });
        match Id::checked(&id) {
            Ok(id) => html! { li id=(id) { (self.flow(&footnote.blocks)) (back) } }.render(out),
            Err(_) => html! { li { (self.flow(&footnote.blocks)) } }.render(out),
        }
    }

    fn subject(&self) -> Subject {
        Subject::File(self.page.file.clone())
    }
}
