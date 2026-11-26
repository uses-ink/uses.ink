//! Rewriting a document: [`Fold`] has one method per node kind, each taking the node and
//! returning its replacement, defaulting to the node rebuilt from its folded children.

use crate::{Block, Definition, Document, Footnote, ImageSource, Inline, Item, LinkTarget};

/// A rewrite of a document. Every method defaults to rebuilding the node from its children,
/// each folded through the node's own `fold`; a fold overrides only the nodes it changes.
///
/// ```
/// use uses_ink_model::{Document, Fold, Inline};
///
/// /// Upper-cases every text.
/// struct Shout;
///
/// impl Fold for Shout {
///     fn text(&mut self, text: String) -> Inline {
///         Inline::Text(text.to_uppercase())
///     }
/// }
///
/// assert_eq!(Shout.document(Document::default()), Document::default());
/// ```
pub trait Fold {
    /// A whole document: its blocks, then its footnotes in order.
    fn document(&mut self, document: Document) -> Document {
        document.fold(self)
    }

    /// One footnote's blocks.
    fn footnote(&mut self, footnote: Footnote) -> Footnote {
        footnote.fold(self)
    }

    /// A run of blocks, each in order. Override to drop or split blocks.
    fn blocks(&mut self, blocks: Vec<Block>) -> Vec<Block> {
        blocks.into_iter().map(|block| self.block(block)).collect()
    }

    /// One block and what it holds.
    fn block(&mut self, block: Block) -> Block {
        block.fold(self)
    }

    /// One list item's blocks; its task state is kept.
    fn item(&mut self, item: Item) -> Item {
        item.fold(self)
    }

    /// One term of a definition list, then each of its details.
    fn definition(&mut self, definition: Definition) -> Definition {
        definition.fold(self)
    }

    /// A run of inlines, each in order. Override to drop or split inlines.
    fn inlines(&mut self, inlines: Vec<Inline>) -> Vec<Inline> {
        inlines.into_iter().map(|inline| self.inline(inline)).collect()
    }

    /// One inline and what it holds.
    fn inline(&mut self, inline: Inline) -> Inline {
        inline.fold(self)
    }

    /// A link, as any inline: link resolution returns it with a resolved target.
    fn link(&mut self, target: LinkTarget, children: Vec<Inline>) -> Inline {
        Inline::Link { target, children: self.inlines(children) }
    }

    /// An image, as any inline.
    fn image(&mut self, source: ImageSource, alt: String) -> Inline {
        Inline::Image { source, alt }
    }

    /// Plain text, as any inline.
    fn text(&mut self, text: String) -> Inline {
        Inline::Text(text)
    }
}

impl Document {
    /// The document with its blocks and each footnote folded.
    #[must_use]
    pub fn fold<F: Fold + ?Sized>(self, fold: &mut F) -> Self {
        let blocks = fold.blocks(self.blocks);
        let footnotes = self.footnotes.into_iter().map(|footnote| fold.footnote(footnote)).collect();
        Self { blocks, footnotes }
    }
}

impl Footnote {
    /// The footnote with its blocks folded.
    #[must_use]
    pub fn fold<F: Fold + ?Sized>(self, fold: &mut F) -> Self {
        Self { name: self.name, blocks: fold.blocks(self.blocks) }
    }
}

impl Item {
    /// The item with its blocks folded.
    #[must_use]
    pub fn fold<F: Fold + ?Sized>(self, fold: &mut F) -> Self {
        Self { task: self.task, blocks: fold.blocks(self.blocks) }
    }
}

impl Definition {
    /// The definition with its term and each run of details folded.
    #[must_use]
    pub fn fold<F: Fold + ?Sized>(self, fold: &mut F) -> Self {
        let term = fold.inlines(self.term);
        let details = self.details.into_iter().map(|details| fold.blocks(details)).collect();
        Self { term, details }
    }
}

impl Block {
    /// The block with what it holds folded, in reading order (as [`Block::walk`]).
    #[must_use]
    pub fn fold<F: Fold + ?Sized>(self, fold: &mut F) -> Self {
        match self {
            Self::Heading { level, anchor, inlines } => Self::Heading { level, anchor, inlines: fold.inlines(inlines) },
            Self::Paragraph(inlines) => Self::Paragraph(fold.inlines(inlines)),
            Self::List { ordered, items, tight } => {
                Self::List { ordered, items: items.into_iter().map(|item| fold.item(item)).collect(), tight }
            }
            Self::Quote(blocks) => Self::Quote(fold.blocks(blocks)),
            Self::Table { alignments, head, rows } => {
                let head = head.into_iter().map(|cell| fold.inlines(cell)).collect();
                let rows =
                    rows.into_iter().map(|row| row.into_iter().map(|cell| fold.inlines(cell)).collect()).collect();
                Self::Table { alignments, head, rows }
            }
            Self::Callout { kind, title, blocks } => {
                let title = title.map(|title| fold.inlines(title));
                Self::Callout { kind, title, blocks: fold.blocks(blocks) }
            }
            Self::Definitions(definitions) => {
                Self::Definitions(definitions.into_iter().map(|definition| fold.definition(definition)).collect())
            }
            Self::Details { summary, blocks } => {
                let summary = fold.inlines(summary);
                Self::Details { summary, blocks: fold.blocks(blocks) }
            }
            leaf @ (Self::Code { .. } | Self::Rule | Self::Math(_)) => leaf,
        }
    }
}

impl Inline {
    /// The inline folded: text through [`Fold::text`], a link through [`Fold::link`], an
    /// image through [`Fold::image`], the children of the others through [`Fold::inlines`].
    #[must_use]
    pub fn fold<F: Fold + ?Sized>(self, fold: &mut F) -> Self {
        match self {
            Self::Text(text) => fold.text(text),
            Self::Link { target, children } => fold.link(target, children),
            Self::Image { source, alt } => fold.image(source, alt),
            Self::Emphasis(children) => Self::Emphasis(fold.inlines(children)),
            Self::Strong(children) => Self::Strong(fold.inlines(children)),
            Self::Strike(children) => Self::Strike(fold.inlines(children)),
            Self::Highlight(children) => Self::Highlight(fold.inlines(children)),
            Self::Superscript(children) => Self::Superscript(fold.inlines(children)),
            Self::Subscript(children) => Self::Subscript(fold.inlines(children)),
            leaf @ (Self::Code(_)
            | Self::Math(_)
            | Self::Keyboard(_)
            | Self::Break
            | Self::SoftBreak
            | Self::Footnote(_)) => leaf,
        }
    }
}
