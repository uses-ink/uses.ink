//! Reading a document: [`Visit`] has one method per node kind, each defaulting to a walk of the
//! node's children, so a visitor overrides only the nodes it cares about.

use crate::{Block, Definition, Document, Footnote, ImageSource, Inline, Item, LinkTarget};

/// A reader of a document. Every method defaults to visiting the node's children through the
/// node's own `walk`; overriding a method and calling `walk` from it keeps the descent.
///
/// ```
/// use uses_ink_model::{Document, Visit};
///
/// /// Counts the words of plain text.
/// #[derive(Default)]
/// struct Words(usize);
///
/// impl Visit for Words {
///     fn text(&mut self, text: &str) {
///         self.0 += text.split_whitespace().count();
///     }
/// }
///
/// let mut words = Words::default();
/// words.document(&Document::default());
/// assert_eq!(words.0, 0);
/// ```
pub trait Visit {
    /// A whole document: its blocks, then its footnotes in order.
    fn document(&mut self, document: &Document) {
        document.walk(self);
    }

    /// One footnote's blocks.
    fn footnote(&mut self, footnote: &Footnote) {
        footnote.walk(self);
    }

    /// A run of blocks, each in order.
    fn blocks(&mut self, blocks: &[Block]) {
        for block in blocks {
            self.block(block);
        }
    }

    /// One block and what it holds.
    fn block(&mut self, block: &Block) {
        block.walk(self);
    }

    /// One list item's blocks; its task state is on `item`.
    fn item(&mut self, item: &Item) {
        item.walk(self);
    }

    /// One term of a definition list, then each of its details.
    fn definition(&mut self, definition: &Definition) {
        definition.walk(self);
    }

    /// A run of inlines, each in order: a paragraph, a heading, a table cell, a link's text.
    fn inlines(&mut self, inlines: &[Inline]) {
        for inline in inlines {
            self.inline(inline);
        }
    }

    /// One inline and what it holds.
    fn inline(&mut self, inline: &Inline) {
        inline.walk(self);
    }

    /// A link and its text.
    fn link(&mut self, _target: &LinkTarget, children: &[Inline]) {
        self.inlines(children);
    }

    /// An image and its alt text. A leaf.
    fn image(&mut self, _source: &ImageSource, _alt: &str) {}

    /// Plain text. A leaf.
    fn text(&mut self, _text: &str) {}
}

impl Document {
    /// Visits the blocks, then each footnote.
    pub fn walk<V: Visit + ?Sized>(&self, visit: &mut V) {
        visit.blocks(&self.blocks);
        for footnote in &self.footnotes {
            visit.footnote(footnote);
        }
    }
}

impl Footnote {
    /// Visits the footnote's blocks.
    pub fn walk<V: Visit + ?Sized>(&self, visit: &mut V) {
        visit.blocks(&self.blocks);
    }
}

impl Item {
    /// Visits the item's blocks.
    pub fn walk<V: Visit + ?Sized>(&self, visit: &mut V) {
        visit.blocks(&self.blocks);
    }
}

impl Definition {
    /// Visits the term, then each run of details.
    pub fn walk<V: Visit + ?Sized>(&self, visit: &mut V) {
        visit.inlines(&self.term);
        for details in &self.details {
            visit.blocks(details);
        }
    }
}

impl Block {
    /// Visits what the block holds, in reading order: a table's head cells, then each row's; a
    /// callout's or a details' title, then its blocks; an embed's blocks. Code, math and rules
    /// hold no nodes.
    pub fn walk<V: Visit + ?Sized>(&self, visit: &mut V) {
        match self {
            Self::Heading { inlines, .. } | Self::Paragraph(inlines) => visit.inlines(inlines),
            Self::List { items, .. } => items.iter().for_each(|item| visit.item(item)),
            Self::Quote(blocks) | Self::Embed { blocks, .. } => visit.blocks(blocks),
            Self::Table { head, rows, .. } => {
                head.iter().chain(rows.iter().flatten()).for_each(|cell| visit.inlines(cell));
            }
            Self::Callout { title, blocks, .. } => {
                if let Some(title) = title {
                    visit.inlines(title);
                }
                visit.blocks(blocks);
            }
            Self::Definitions(definitions) => definitions.iter().for_each(|definition| visit.definition(definition)),
            Self::Details { summary, blocks } => {
                visit.inlines(summary);
                visit.blocks(blocks);
            }
            Self::Code { .. } | Self::Rule | Self::Math(_) => {}
        }
    }
}

impl Inline {
    /// Visits what the inline holds: text through [`Visit::text`], a link through
    /// [`Visit::link`], an image through [`Visit::image`], the children of emphasis, strong,
    /// strike, highlight, superscript and subscript through [`Visit::inlines`]. Code, math,
    /// keys, breaks, footnote references and anchors hold no nodes.
    pub fn walk<V: Visit + ?Sized>(&self, visit: &mut V) {
        match self {
            Self::Text(text) => visit.text(text),
            Self::Link { target, children } => visit.link(target, children),
            Self::Image { source, alt } => visit.image(source, alt),
            Self::Emphasis(children)
            | Self::Strong(children)
            | Self::Strike(children)
            | Self::Highlight(children)
            | Self::Superscript(children)
            | Self::Subscript(children) => visit.inlines(children),
            Self::Code(_)
            | Self::Math(_)
            | Self::Keyboard(_)
            | Self::Break
            | Self::SoftBreak
            | Self::Footnote(_)
            | Self::Anchor(_) => {}
        }
    }
}
