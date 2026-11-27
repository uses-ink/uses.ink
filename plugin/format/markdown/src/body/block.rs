//! Blocks: how a comrak block joins its run, and the blocks that need the walk to build.

use comrak::nodes::{
    AlertType, AstNode, CalloutFold, ListType, NodeAlert, NodeCallout, NodeCodeBlock, NodeTaskItem, NodeValue,
    TableAlignment,
};
use comrak::options::AlertStyleType;
use model::{
    Alignment, Anchor, Announcement, Block, CalloutKind, Definition, Folding, Footnote, FootnoteName, Inline, Item,
    Level, LinkTarget, Slug, Span, Spanned, Task,
};

use super::html::Fold;
use super::inline::Plain;
use super::stack::Stack;
use super::{Builder, Literal, Node, ToModel};
use crate::dialect::{Dialect, Extension};

impl Node for Block {
    type Open = Fold;

    fn read<'a, D: Dialect>(child: &'a AstNode<'a>, builder: &mut Builder<'_, D>, run: &mut Stack<Fold>) {
        let data = child.data();
        let span = builder.positions.span(data.sourcepos);
        let block = match &data.value {
            NodeValue::HtmlBlock(html) => return builder.html(&html.literal, span, run),
            NodeValue::Paragraph => builder.paragraph(child, span),
            NodeValue::Heading(heading) => Some(builder.heading(child, heading.level, span)),
            NodeValue::List(list) => {
                let ordered = match list.list_type {
                    ListType::Ordered => Some(list.start as u64),
                    ListType::Bullet => None,
                };
                Some(Self::List { ordered, items: builder.items(child), tight: list.tight })
            }
            NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => Some(Self::Quote(builder.run(child))),
            NodeValue::CodeBlock(code) => Some(code.to_model()),
            NodeValue::ThematicBreak(_) => Some(Self::Rule),
            NodeValue::Table(table) => Some(builder.table(child, &table.alignments)),
            NodeValue::Alert(alert) => Some(builder.alert(child, alert, span)),
            NodeValue::Callout(callout) => Some(builder.callout(child, callout, span)),
            // read by the callout it titles
            NodeValue::CalloutTitle => None,
            NodeValue::DescriptionList => Some(builder.definitions(child)),
            NodeValue::MathBlock(math) => Some(Self::Math(math.literal.trim_end_matches('\n').to_owned())),
            NodeValue::FootnoteDefinition(definition) => {
                builder.footnote(child, &definition.name, span);
                None
            }
            _ => {
                for block in builder.run(child) {
                    run.push(block);
                }
                None
            }
        };
        if let Some(block) = block {
            run.push(block);
        }
    }

    /// A paragraph of the children's text, each child's text joined by a space.
    fn flatten<'a>(parent: &'a AstNode<'a>) -> Option<Self> {
        let text = parent.children().map(|child| Literal(child).to_string()).collect::<Vec<_>>().join(" ");
        let text = text.trim();
        (!text.is_empty()).then(|| Self::Paragraph(vec![Inline::Text(text.to_owned())]))
    }

    fn join(run: &mut Vec<Self>, block: Self) {
        run.push(block);
    }

    fn shows(run: &[Self]) -> bool {
        !run.is_empty()
    }
}

impl<D: Dialect> Builder<'_, D> {
    /// A paragraph, or nothing when it shows nothing. What it holds alone can make it another
    /// block: display math when `$$...$$`, an embed when a wikilink embeds a page, an anchor of
    /// the block before it when a block id (`^id`) is all its text.
    fn paragraph<'a>(&mut self, node: &'a AstNode<'a>, span: Span) -> Option<Block> {
        let mut children = node.children();
        if let (Some(only), None) = (children.next(), children.next()) {
            match &only.data().value {
                NodeValue::Math(math) if math.display_math => return Some(Block::Math(math.literal.clone())),
                NodeValue::WikiLink(link) => {
                    let linked = self.wiki(only, link, span);
                    return Some(match linked.embed {
                        Some(written) => {
                            Block::Embed { target: LinkTarget::Unresolved { written }, blocks: Vec::new() }
                        }
                        None => Block::Paragraph(vec![linked.inline]),
                    });
                }
                NodeValue::Text(text) if D::SYNTAX.has(Extension::BlockIds) => {
                    if let Some(anchor) = text.trim().strip_prefix('^').and_then(|name| self.standalone(name, span)) {
                        return Some(Block::Paragraph(vec![Inline::Anchor(anchor)]));
                    }
                }
                _ => {}
            }
        }
        let inlines: Vec<Inline> = self.run(node);
        Inline::shows(&inlines).then_some(Block::Paragraph(inlines))
    }

    /// The anchor of a block id alone on its line, `name` the text after its `^`, or `None` when
    /// that text is no block id: Obsidian's ids are ASCII letters, digits and `-`.
    fn standalone(&mut self, name: &str, span: Span) -> Option<Anchor> {
        let id = !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
        id.then(|| self.names.block(name, span, self.report)).flatten()
    }

    /// A heading with its anchor; the first level-1 heading with text is also the body's
    /// heading.
    fn heading<'a>(&mut self, node: &'a AstNode<'a>, level: u8, span: Span) -> Block {
        let inlines = self.run(node);
        let text = Plain::of(&inlines);
        let text = text.trim();
        let anchor = self.names.name::<Anchor>(text, Some(span), self.report);
        let (Ok(level), Some(anchor)) = (Level::new(level), anchor) else {
            return Block::Paragraph(inlines);
        };
        if level.get() == 1 && self.heading.is_none() && !text.is_empty() {
            self.heading = Some(Spanned::new(text.to_owned(), span));
        }
        Block::Heading { level, anchor, inlines }
    }

    /// The items of a list, a task list's with their state.
    fn items<'a>(&mut self, node: &'a AstNode<'a>) -> Vec<Item> {
        node.children()
            .map(|item| match &item.data().value {
                NodeValue::TaskItem(task) => Item::task(task.to_model(), self.run(item)),
                _ => Item::new(self.run(item)),
            })
            .collect()
    }

    /// A table, every row with one cell per column.
    fn table<'a>(&mut self, node: &'a AstNode<'a>, alignments: &[TableAlignment]) -> Block {
        let alignments: Vec<Alignment> = alignments.iter().map(ToModel::to_model).collect();
        let mut head = Vec::new();
        let mut rows = Vec::new();
        for row in node.children() {
            let header = matches!(&row.data().value, NodeValue::TableRow(row) if row.header);
            let mut cells: Vec<Vec<Inline>> = row.children().map(|cell| self.run(cell)).collect();
            cells.resize_with(alignments.len(), Vec::new);
            if header && head.is_empty() { head = cells } else { rows.push(cells) }
        }
        if head.is_empty() {
            head.resize_with(alignments.len(), Vec::new);
        }
        Block::Table { alignments, head, rows }
    }

    /// A GitHub alert (`> [!note]`), as a callout. A kind comrak learns later keeps the name
    /// comrak gives it.
    fn alert<'a>(&mut self, node: &'a AstNode<'a>, alert: &NodeAlert, span: Span) -> Block {
        let kind = alert.alert_type.to_model().unwrap_or_else(|| {
            let name = alert.alert_type.css_class(AlertStyleType::Semantic);
            self.kind(name, span)
        });
        let title = alert.title.as_ref().map(|title| vec![Inline::Text(title.clone())]);
        Block::Callout { kind, title, blocks: self.run(node), folding: Folding::Fixed }
    }

    /// An Obsidian callout (`> [!tip]- Title`): its kind as the dialect names it, its title of
    /// inlines, and whether it folds. The metadata after a `|` styles a callout in Obsidian's
    /// own stylesheets; nothing here reads it, and that is said.
    fn callout<'a>(&mut self, node: &'a AstNode<'a>, callout: &NodeCallout, span: Span) -> Block {
        let written = callout.callout_type.as_str();
        let kind = self.dialect.callout(written).unwrap_or_else(|| self.kind(written, span));
        if let Some(metadata) = &callout.metadata {
            self.report.announce(Announcement::CalloutMetadataIgnored { written: metadata.clone() }, span);
        }
        let title = node
            .first_child()
            .filter(|first| matches!(first.data().value, NodeValue::CalloutTitle))
            .map(|first| self.term(first))
            .filter(|title| Inline::shows(title));
        let folding = match callout.fold {
            CalloutFold::None => Folding::Fixed,
            CalloutFold::Expanded => Folding::Open,
            CalloutFold::Collapsed => Folding::Closed,
        };
        Block::Callout { kind, title, blocks: self.run(node), folding }
    }

    /// A callout kind no dialect names: its own name as a slug, or a note when the name is no
    /// slug, which is said.
    fn kind(&mut self, name: &str, span: Span) -> CalloutKind {
        if let Ok(slug) = Slug::parse(name) {
            return CalloutKind::Other(slug);
        }
        self.report.announce(Announcement::CalloutKindUnknown { kind: name.to_owned() }, span);
        CalloutKind::Note
    }

    /// A description list. comrak gives a term written once with several `:` details as one
    /// item with the term and items without one; those details join the term before them.
    fn definitions<'a>(&mut self, node: &'a AstNode<'a>) -> Block {
        let mut definitions: Vec<Definition> = Vec::new();
        for item in node.children() {
            let mut term = None;
            let mut details = Vec::new();
            for part in item.children() {
                match &part.data().value {
                    NodeValue::DescriptionTerm => term = Some(self.term(part)),
                    // comrak puts only terms and details in an item
                    _ => details.push(self.run(part)),
                }
            }
            match (term, definitions.last_mut()) {
                (None, Some(last)) => last.details.extend(details),
                (Some(term), _) => definitions.push(Definition::new(term, details)),
                (None, None) => definitions.push(Definition::new(Vec::new(), details)),
            }
        }
        Block::Definitions(definitions)
    }

    /// A term's inlines: comrak wraps them in paragraphs, joined here by soft breaks.
    fn term<'a>(&mut self, node: &'a AstNode<'a>) -> Vec<Inline> {
        let mut inlines = Vec::new();
        for block in self.run(node) {
            if let Block::Paragraph(paragraph) = block {
                if !inlines.is_empty() {
                    inlines.push(Inline::SoftBreak);
                }
                inlines.extend(paragraph);
            }
        }
        inlines
    }

    /// A footnote definition: not part of the flow, it joins the document's footnotes.
    fn footnote<'a>(&mut self, node: &'a AstNode<'a>, given: &str, span: Span) {
        let blocks = self.run(node);
        if let Some(name) = self.names.name::<FootnoteName>(given, Some(span), self.report) {
            self.footnotes.push(Footnote::new(name, blocks));
        }
    }
}

impl ToModel for NodeCodeBlock {
    type Model = Block;

    /// Code with its language, or display math for a ` ```math ` block.
    fn to_model(&self) -> Block {
        let language = self.info.split_whitespace().next().map(str::to_owned);
        if language.as_deref() == Some("math") {
            return Block::Math(self.literal.trim_end_matches('\n').to_owned());
        }
        Block::Code { language, text: self.literal.clone() }
    }
}

impl ToModel for TableAlignment {
    type Model = Alignment;

    fn to_model(&self) -> Alignment {
        match self {
            Self::None => Alignment::None,
            Self::Left => Alignment::Left,
            Self::Center => Alignment::Center,
            Self::Right => Alignment::Right,
        }
    }
}

impl ToModel for NodeTaskItem {
    type Model = Task;

    fn to_model(&self) -> Task {
        if self.symbol.is_some() { Task::Done } else { Task::Open }
    }
}

impl ToModel for AlertType {
    type Model = Option<CalloutKind>;

    /// The kind, for the five GitHub names; `None` for a kind comrak learns later.
    fn to_model(&self) -> Option<CalloutKind> {
        Some(match self {
            Self::Note => CalloutKind::Note,
            Self::Tip => CalloutKind::Tip,
            Self::Important => CalloutKind::Important,
            Self::Warning => CalloutKind::Warning,
            Self::Caution => CalloutKind::Caution,
            _ => return None,
        })
    }
}
