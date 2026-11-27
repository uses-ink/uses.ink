// Documents, flattened into WIT's lists and rebuilt from them. Part of `wit/mirror`.

use model::{
    Alignment, Block, CalloutKind, Definition, Document, Folding, Footnote, Hash, ImageSource, Inline, Item, Level,
    LinkTarget, PageId, Task,
};

use super::refusal::{Malformed, Unmirrored};
use super::wit::model::document as flat;
use super::written::{Bytes, Written};

/// A document flattened into WIT's lists: each node pushed after its children.
#[derive(Default)]
struct Flat {
    blocks: Vec<flat::Block>,
    inlines: Vec<flat::Inline>,
}

impl Flat {
    fn place(length: usize) -> Result<u32, Unmirrored> {
        u32::try_from(length).map_err(|_| Unmirrored::Large)
    }

    fn inlines(&mut self, inlines: &[Inline]) -> Result<Vec<u32>, Unmirrored> {
        inlines.iter().map(|inline| self.inline(inline)).collect()
    }

    fn blocks(&mut self, blocks: &[Block]) -> Result<Vec<u32>, Unmirrored> {
        blocks.iter().map(|block| self.block(block)).collect()
    }

    fn inline(&mut self, inline: &Inline) -> Result<u32, Unmirrored> {
        let node = match inline {
            Inline::Text(text) => flat::Inline::Text(text.clone()),
            Inline::Emphasis(children) => flat::Inline::Emphasis(self.inlines(children)?),
            Inline::Strong(children) => flat::Inline::Strong(self.inlines(children)?),
            Inline::Strike(children) => flat::Inline::Strike(self.inlines(children)?),
            Inline::Code(code) => flat::Inline::Code(code.clone()),
            Inline::Link { target, children } => {
                let children = self.inlines(children)?;
                flat::Inline::Link(flat::Link { target: target.try_into()?, children })
            }
            Inline::Image { source, alt } => {
                flat::Inline::Image(flat::Image { source: source.try_into()?, alt: alt.clone() })
            }
            Inline::Break => flat::Inline::Break,
            Inline::SoftBreak => flat::Inline::SoftBreak,
            Inline::Math(tex) => flat::Inline::Math(tex.clone()),
            Inline::Footnote(name) => flat::Inline::Footnote(name.to_string()),
            Inline::Highlight(children) => flat::Inline::Highlight(self.inlines(children)?),
            Inline::Superscript(children) => flat::Inline::Superscript(self.inlines(children)?),
            Inline::Subscript(children) => flat::Inline::Subscript(self.inlines(children)?),
            Inline::Keyboard(keys) => flat::Inline::Keyboard(keys.clone()),
            Inline::Anchor(anchor) => flat::Inline::Anchor(anchor.to_string()),
            _ => return Err(Unmirrored::Inline),
        };
        let at = Self::place(self.inlines.len())?;
        self.inlines.push(node);
        Ok(at)
    }

    fn block(&mut self, block: &Block) -> Result<u32, Unmirrored> {
        let node = match block {
            Block::Heading { level, anchor, inlines } => flat::Block::Heading(flat::Heading {
                level: level.get(),
                anchor: anchor.to_string(),
                inlines: self.inlines(inlines)?,
            }),
            Block::Paragraph(inlines) => flat::Block::Paragraph(self.inlines(inlines)?),
            Block::List { ordered, items, tight } => {
                let items = items.iter().map(|item| self.item(item)).collect::<Result<_, _>>()?;
                flat::Block::List(flat::Listed { ordered: *ordered, items, tight: *tight })
            }
            Block::Quote(blocks) => flat::Block::Quote(self.blocks(blocks)?),
            Block::Code { language, text } => {
                flat::Block::Code(flat::Code { language: language.clone(), text: text.clone() })
            }
            Block::Rule => flat::Block::Rule,
            Block::Table { alignments, head, rows } => flat::Block::Table(self.table(alignments, head, rows)?),
            Block::Callout { kind, title, blocks, folding } => {
                let title = title.as_deref().map(|title| self.inlines(title)).transpose()?;
                let blocks = self.blocks(blocks)?;
                let folding = flat::Folding::try_from(*folding)?;
                flat::Block::Callout(flat::Callout { kind: kind.try_into()?, title, blocks, folding })
            }
            Block::Math(tex) => flat::Block::Math(tex.clone()),
            Block::Definitions(definitions) => {
                let definitions = definitions.iter().map(|d| self.definition(d)).collect::<Result<_, _>>()?;
                flat::Block::Definitions(definitions)
            }
            Block::Details { summary, blocks } => {
                let summary = self.inlines(summary)?;
                flat::Block::Details(flat::Details { summary, blocks: self.blocks(blocks)? })
            }
            Block::Embed { target, blocks } => {
                flat::Block::Embed(flat::Embed { target: target.try_into()?, blocks: self.blocks(blocks)? })
            }
            _ => return Err(Unmirrored::Block),
        };
        let at = Self::place(self.blocks.len())?;
        self.blocks.push(node);
        Ok(at)
    }

    fn table(
        &mut self,
        alignments: &[Alignment],
        head: &[Vec<Inline>],
        rows: &[Vec<Vec<Inline>>],
    ) -> Result<flat::Table, Unmirrored> {
        let alignments = alignments.iter().map(|a| flat::Alignment::try_from(*a)).collect::<Result<_, _>>()?;
        let head = head.iter().map(|cell| self.inlines(cell)).collect::<Result<_, _>>()?;
        let rows = rows
            .iter()
            .map(|row| row.iter().map(|cell| self.inlines(cell)).collect::<Result<_, _>>())
            .collect::<Result<_, _>>()?;
        Ok(flat::Table { alignments, head, rows })
    }

    fn item(&mut self, item: &Item) -> Result<flat::Item, Unmirrored> {
        let task = item.task.map(|task| match task {
            Task::Open => flat::Task::Open,
            Task::Done => flat::Task::Done,
        });
        Ok(flat::Item { task, blocks: self.blocks(&item.blocks)? })
    }

    fn definition(&mut self, definition: &Definition) -> Result<flat::Definition, Unmirrored> {
        let term = self.inlines(&definition.term)?;
        let details = definition.details.iter().map(|d| self.blocks(d)).collect::<Result<_, _>>()?;
        Ok(flat::Definition { term, details })
    }
}

impl TryFrom<&LinkTarget> for flat::LinkTarget {
    type Error = Unmirrored;

    fn try_from(target: &LinkTarget) -> Result<Self, Unmirrored> {
        Ok(match target {
            LinkTarget::Page { id, anchor } => Self::Page(flat::PageLink {
                id: id.hash().as_bytes().to_vec(),
                anchor: anchor.as_ref().map(ToString::to_string),
            }),
            LinkTarget::Anchor(anchor) => Self::Anchor(anchor.to_string()),
            LinkTarget::External(url) => Self::External(url.to_string()),
            LinkTarget::Unresolved { written } => Self::Unresolved(written.clone()),
            _ => return Err(Unmirrored::Link),
        })
    }
}

impl TryFrom<&ImageSource> for flat::ImageSource {
    type Error = Unmirrored;

    fn try_from(source: &ImageSource) -> Result<Self, Unmirrored> {
        Ok(match source {
            ImageSource::Asset(path) => Self::Asset(path.to_string()),
            ImageSource::External(url) => Self::External(url.to_string()),
            ImageSource::Unresolved { written } => Self::Unresolved(written.clone()),
            _ => return Err(Unmirrored::Image),
        })
    }
}

impl TryFrom<Alignment> for flat::Alignment {
    type Error = Unmirrored;

    fn try_from(alignment: Alignment) -> Result<Self, Unmirrored> {
        Ok(match alignment {
            Alignment::None => Self::None,
            Alignment::Left => Self::Left,
            Alignment::Center => Self::Center,
            Alignment::Right => Self::Right,
            _ => return Err(Unmirrored::Alignment),
        })
    }
}

impl TryFrom<Folding> for flat::Folding {
    type Error = Unmirrored;

    fn try_from(folding: Folding) -> Result<Self, Unmirrored> {
        Ok(match folding {
            Folding::Fixed => Self::Fixed,
            Folding::Open => Self::Open,
            Folding::Closed => Self::Closed,
            _ => return Err(Unmirrored::Callout),
        })
    }
}

impl TryFrom<&CalloutKind> for flat::CalloutKind {
    type Error = Unmirrored;

    fn try_from(kind: &CalloutKind) -> Result<Self, Unmirrored> {
        Ok(match kind {
            CalloutKind::Note => Self::Note,
            CalloutKind::Tip => Self::Tip,
            CalloutKind::Important => Self::Important,
            CalloutKind::Warning => Self::Warning,
            CalloutKind::Caution => Self::Caution,
            CalloutKind::Other(slug) => Self::Other(slug.to_string()),
            _ => return Err(Unmirrored::Callout),
        })
    }
}

impl TryFrom<&Document> for flat::Document {
    type Error = Unmirrored;

    fn try_from(document: &Document) -> Result<Self, Unmirrored> {
        let mut flat = Flat::default();
        let body = flat.blocks(&document.blocks)?;
        let footnotes = document
            .footnotes
            .iter()
            .map(|note| Ok(flat::Footnote { name: note.name.to_string(), blocks: flat.blocks(&note.blocks)? }))
            .collect::<Result<_, Unmirrored>>()?;
        Ok(Self { blocks: flat.blocks, inlines: flat.inlines, body, footnotes })
    }
}

/// A flattened document rebuilt: each node built in list order from children built before it,
/// each child taken out of its slot, so a node can have one parent only.
struct Lift {
    blocks: Vec<Option<Block>>,
    inlines: Vec<Option<Inline>>,
}

impl Lift {
    /// The inlines at `places`, each before `before` when given.
    fn inlines(&mut self, places: Vec<u32>, before: Option<usize>) -> Result<Vec<Inline>, Malformed> {
        places.into_iter().map(|at| Self::take(&mut self.inlines, at, before)).collect()
    }

    /// The blocks at `places`, each before `before`.
    fn blocks(&mut self, places: Vec<u32>, before: usize) -> Result<Vec<Block>, Malformed> {
        places.into_iter().map(|at| Self::take(&mut self.blocks, at, Some(before))).collect()
    }

    fn take<T>(slots: &mut [Option<T>], at: u32, before: Option<usize>) -> Result<T, Malformed> {
        let place = usize::try_from(at).map_err(|_| Malformed::Child { at })?;
        if before.is_some_and(|before| place >= before) {
            return Err(Malformed::Child { at });
        }
        let slot = slots.get_mut(place).ok_or(Malformed::Child { at })?;
        slot.take().ok_or(Malformed::Shared { at })
    }

    fn inline(&mut self, inline: flat::Inline, at: usize) -> Result<Inline, Malformed> {
        let before = Some(at);
        Ok(match inline {
            flat::Inline::Text(text) => Inline::Text(text),
            flat::Inline::Emphasis(children) => Inline::Emphasis(self.inlines(children, before)?),
            flat::Inline::Strong(children) => Inline::Strong(self.inlines(children, before)?),
            flat::Inline::Strike(children) => Inline::Strike(self.inlines(children, before)?),
            flat::Inline::Code(code) => Inline::Code(code),
            flat::Inline::Link(link) => {
                Inline::Link { target: link.target.try_into()?, children: self.inlines(link.children, before)? }
            }
            flat::Inline::Image(image) => Inline::Image { source: image.source.try_into()?, alt: image.alt },
            flat::Inline::Break => Inline::Break,
            flat::Inline::SoftBreak => Inline::SoftBreak,
            flat::Inline::Math(tex) => Inline::Math(tex),
            flat::Inline::Footnote(name) => Inline::Footnote(Written::read(name)?),
            flat::Inline::Highlight(children) => Inline::Highlight(self.inlines(children, before)?),
            flat::Inline::Superscript(children) => Inline::Superscript(self.inlines(children, before)?),
            flat::Inline::Subscript(children) => Inline::Subscript(self.inlines(children, before)?),
            flat::Inline::Keyboard(keys) => Inline::Keyboard(keys),
            flat::Inline::Anchor(anchor) => Inline::Anchor(Written::read(anchor)?),
        })
    }

    fn block(&mut self, block: flat::Block, at: usize) -> Result<Block, Malformed> {
        Ok(match block {
            flat::Block::Heading(heading) => Block::Heading {
                level: Level::new(heading.level).map_err(|source| Malformed::Level { source })?,
                anchor: Written::read(heading.anchor)?,
                inlines: self.inlines(heading.inlines, None)?,
            },
            flat::Block::Paragraph(inlines) => Block::Paragraph(self.inlines(inlines, None)?),
            flat::Block::List(listed) => {
                let items = listed.items.into_iter().map(|item| self.item(item, at)).collect::<Result<_, _>>()?;
                Block::List { ordered: listed.ordered, items, tight: listed.tight }
            }
            flat::Block::Quote(blocks) => Block::Quote(self.blocks(blocks, at)?),
            flat::Block::Code(code) => Block::Code { language: code.language, text: code.text },
            flat::Block::Rule => Block::Rule,
            flat::Block::Table(table) => self.table(table)?,
            flat::Block::Callout(callout) => Block::Callout {
                kind: callout.kind.try_into()?,
                title: callout.title.map(|title| self.inlines(title, None)).transpose()?,
                blocks: self.blocks(callout.blocks, at)?,
                folding: callout.folding.into(),
            },
            flat::Block::Math(tex) => Block::Math(tex),
            flat::Block::Definitions(definitions) => Block::Definitions(
                definitions.into_iter().map(|d| self.definition(d, at)).collect::<Result<_, _>>()?,
            ),
            flat::Block::Details(details) => Block::Details {
                summary: self.inlines(details.summary, None)?,
                blocks: self.blocks(details.blocks, at)?,
            },
            flat::Block::Embed(embed) => {
                Block::Embed { target: embed.target.try_into()?, blocks: self.blocks(embed.blocks, at)? }
            }
        })
    }

    fn table(&mut self, table: flat::Table) -> Result<Block, Malformed> {
        let alignments = table.alignments.into_iter().map(Alignment::from).collect();
        let head = table.head.into_iter().map(|cell| self.inlines(cell, None)).collect::<Result<_, _>>()?;
        let rows = table
            .rows
            .into_iter()
            .map(|row| row.into_iter().map(|cell| self.inlines(cell, None)).collect::<Result<_, _>>())
            .collect::<Result<_, _>>()?;
        Ok(Block::Table { alignments, head, rows })
    }

    fn definition(&mut self, definition: flat::Definition, at: usize) -> Result<Definition, Malformed> {
        let term = self.inlines(definition.term, None)?;
        let details = definition.details.into_iter().map(|b| self.blocks(b, at)).collect::<Result<_, _>>()?;
        Ok(Definition::new(term, details))
    }

    fn item(&mut self, item: flat::Item, at: usize) -> Result<Item, Malformed> {
        let blocks = self.blocks(item.blocks, at)?;
        Ok(match item.task {
            None => Item::new(blocks),
            Some(flat::Task::Open) => Item::task(Task::Open, blocks),
            Some(flat::Task::Done) => Item::task(Task::Done, blocks),
        })
    }

    /// The first slot still holding a node, which no parent named.
    fn unused<T>(slots: &[Option<T>]) -> Result<(), Malformed> {
        match slots.iter().position(Option::is_some) {
            Some(at) => Err(Malformed::Unused { at }),
            None => Ok(()),
        }
    }
}

impl TryFrom<flat::LinkTarget> for LinkTarget {
    type Error = Malformed;

    fn try_from(target: flat::LinkTarget) -> Result<Self, Malformed> {
        Ok(match target {
            flat::LinkTarget::Page(link) => Self::Page {
                id: Hash::try_from(Bytes(link.id)).map(PageId::from)?,
                anchor: Written::maybe(link.anchor)?,
            },
            flat::LinkTarget::Anchor(anchor) => Self::Anchor(Written::read(anchor)?),
            flat::LinkTarget::External(url) => Self::External(Written::read(url)?),
            flat::LinkTarget::Unresolved(written) => Self::Unresolved { written },
        })
    }
}

impl TryFrom<flat::ImageSource> for ImageSource {
    type Error = Malformed;

    fn try_from(source: flat::ImageSource) -> Result<Self, Malformed> {
        Ok(match source {
            flat::ImageSource::Asset(path) => Self::Asset(Written::read(path)?),
            flat::ImageSource::External(url) => Self::External(Written::read(url)?),
            flat::ImageSource::Unresolved(written) => Self::Unresolved { written },
        })
    }
}

impl From<flat::Alignment> for Alignment {
    fn from(alignment: flat::Alignment) -> Self {
        match alignment {
            flat::Alignment::None => Self::None,
            flat::Alignment::Left => Self::Left,
            flat::Alignment::Center => Self::Center,
            flat::Alignment::Right => Self::Right,
        }
    }
}

impl From<flat::Folding> for Folding {
    fn from(folding: flat::Folding) -> Self {
        match folding {
            flat::Folding::Fixed => Self::Fixed,
            flat::Folding::Open => Self::Open,
            flat::Folding::Closed => Self::Closed,
        }
    }
}

impl TryFrom<flat::CalloutKind> for CalloutKind {
    type Error = Malformed;

    fn try_from(kind: flat::CalloutKind) -> Result<Self, Malformed> {
        Ok(match kind {
            flat::CalloutKind::Note => Self::Note,
            flat::CalloutKind::Tip => Self::Tip,
            flat::CalloutKind::Important => Self::Important,
            flat::CalloutKind::Warning => Self::Warning,
            flat::CalloutKind::Caution => Self::Caution,
            flat::CalloutKind::Other(slug) => Self::Other(Written::read(slug)?),
        })
    }
}

impl TryFrom<flat::Document> for Document {
    type Error = Malformed;

    fn try_from(document: flat::Document) -> Result<Self, Malformed> {
        let mut lift = Lift {
            blocks: std::iter::repeat_with(|| None).take(document.blocks.len()).collect(),
            inlines: std::iter::repeat_with(|| None).take(document.inlines.len()).collect(),
        };
        for (at, inline) in document.inlines.into_iter().enumerate() {
            let built = lift.inline(inline, at)?;
            if let Some(slot) = lift.inlines.get_mut(at) {
                *slot = Some(built);
            }
        }
        for (at, block) in document.blocks.into_iter().enumerate() {
            let built = lift.block(block, at)?;
            if let Some(slot) = lift.blocks.get_mut(at) {
                *slot = Some(built);
            }
        }
        let end = lift.blocks.len();
        let body = lift.blocks(document.body, end)?;
        let footnotes = document
            .footnotes
            .into_iter()
            .map(|note| Ok(Footnote::new(Written::read(note.name)?, lift.blocks(note.blocks, end)?)))
            .collect::<Result<_, Malformed>>()?;
        Lift::unused(&lift.blocks)?;
        Lift::unused(&lift.inlines)?;
        Ok(Self::new(body).with_footnotes(footnotes))
    }
}
