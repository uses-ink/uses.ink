//! Blocks.

use comrak::nodes::{AlertType, AstNode, ListType, NodeAlert, NodeCodeBlock, NodeList, NodeValue, TableAlignment};
use comrak::options::AlertStyleType;
use model::{
    Alignment, Announcement, Block, CalloutKind, Definition, Footnote, Inline, Item, Level, Notice, Slug, Span,
    Spanned, Task,
};

use super::fold::Nest;
use super::inline::{has_content, plain};
use super::{DEPTH, Walk, text_of};

impl Walk<'_> {
    pub(super) fn blocks<'a>(&mut self, node: &'a AstNode<'a>) -> Vec<Block> {
        let mut out = Vec::new();
        if self.depth >= DEPTH {
            self.too_deep(node, &mut out);
            return out;
        }
        self.depth += 1;
        let mut nest = Nest::default();
        for child in node.children() {
            self.block(child, &mut nest);
        }
        self.depth -= 1;
        self.unfold(nest)
    }

    fn block<'a>(&mut self, node: &'a AstNode<'a>, nest: &mut Nest) {
        let data = node.data();
        let span = self.span(data.sourcepos);
        if let NodeValue::HtmlBlock(block) = &data.value {
            return self.html_block(&block.literal, span, nest);
        }
        let out = nest.target();
        match &data.value {
            NodeValue::Paragraph => self.paragraph(node, out),
            NodeValue::Heading(heading) => out.push(self.heading(node, heading.level, span)),
            NodeValue::List(list) => out.push(self.list(node, list)),
            NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => out.push(Block::Quote(self.blocks(node))),
            NodeValue::CodeBlock(code) => out.push(code_block(code)),
            NodeValue::ThematicBreak(_) => out.push(Block::Rule),
            NodeValue::Table(table) => out.push(self.table(node, &table.alignments)),
            NodeValue::Alert(alert) => out.push(self.callout(node, alert, span)),
            NodeValue::DescriptionList => out.push(self.definitions(node)),
            NodeValue::MathBlock(math) => out.push(Block::Math(math.literal.trim_end_matches('\n').to_owned())),
            NodeValue::FootnoteDefinition(definition) => {
                // a definition is not part of the flow: it joins the document's footnotes
                let blocks = self.blocks(node);
                if let Some(name) = self.footnote_name(&definition.name, Some(span)) {
                    self.footnotes.push(Footnote::new(name, blocks));
                }
            }
            _ => out.extend(self.blocks(node)),
        }
    }

    /// A paragraph, or display math when `$$...$$` is all it holds.
    fn paragraph<'a>(&mut self, node: &'a AstNode<'a>, out: &mut Vec<Block>) {
        let mut children = node.children();
        if let (Some(only), None) = (children.next(), children.next())
            && let NodeValue::Math(math) = &only.data().value
            && math.display_math
        {
            out.push(Block::Math(math.literal.clone()));
            return;
        }
        let inlines = self.inlines(node);
        if has_content(&inlines) {
            out.push(Block::Paragraph(inlines));
        }
    }

    fn heading<'a>(&mut self, node: &'a AstNode<'a>, level: u8, span: Span) -> Block {
        let inlines = self.inlines(node);
        let text = plain(&inlines);
        let text = text.trim();
        let (Ok(level), Some(anchor)) = (Level::new(level), self.anchor(text, span)) else {
            return Block::Paragraph(inlines);
        };
        if level.get() == 1 && self.heading.is_none() && !text.is_empty() {
            self.heading = Some(Spanned::new(text.to_owned(), span));
        }
        Block::Heading { level, anchor, inlines }
    }

    fn list<'a>(&mut self, node: &'a AstNode<'a>, list: &NodeList) -> Block {
        let ordered = match list.list_type {
            ListType::Ordered => Some(list.start as u64),
            ListType::Bullet => None,
        };
        let items = node
            .children()
            .map(|item| match &item.data().value {
                NodeValue::TaskItem(task) => {
                    let done = if task.symbol.is_some() { Task::Done } else { Task::Open };
                    Item::task(done, self.blocks(item))
                }
                _ => Item::new(self.blocks(item)),
            })
            .collect();
        Block::List { ordered, items, tight: list.tight }
    }

    fn table<'a>(&mut self, node: &'a AstNode<'a>, alignments: &[TableAlignment]) -> Block {
        let alignments: Vec<Alignment> = alignments.iter().map(|&a| alignment(a)).collect();
        let mut head = Vec::new();
        let mut rows = Vec::new();
        for row in node.children() {
            let header = matches!(&row.data().value, NodeValue::TableRow(row) if row.header);
            let mut cells: Vec<Vec<Inline>> = row.children().map(|cell| self.inlines(cell)).collect();
            cells.resize_with(alignments.len(), Vec::new);
            if header && head.is_empty() { head = cells } else { rows.push(cells) }
        }
        if head.is_empty() {
            head.resize_with(alignments.len(), Vec::new);
        }
        Block::Table { alignments, head, rows }
    }

    fn callout<'a>(&mut self, node: &'a AstNode<'a>, alert: &NodeAlert, span: Span) -> Block {
        let kind = match alert.alert_type {
            AlertType::Note => CalloutKind::Note,
            AlertType::Tip => CalloutKind::Tip,
            AlertType::Important => CalloutKind::Important,
            AlertType::Warning => CalloutKind::Warning,
            AlertType::Caution => CalloutKind::Caution,
            // a kind comrak learns later keeps the name comrak gives it
            other => self.callout_kind(other.css_class(AlertStyleType::Semantic), span),
        };
        let title = alert.title.as_ref().map(|title| vec![Inline::Text(title.clone())]);
        Block::Callout { kind, title, blocks: self.blocks(node) }
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
                    _ => details.push(self.blocks(part)),
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

    /// A term's inlines: comrak wraps them in a paragraph.
    fn term<'a>(&mut self, node: &'a AstNode<'a>) -> Vec<Inline> {
        let mut inlines = Vec::new();
        for block in self.blocks(node) {
            if let Block::Paragraph(paragraph) = block {
                if !inlines.is_empty() {
                    inlines.push(Inline::SoftBreak);
                }
                inlines.extend(paragraph);
            }
        }
        inlines
    }

    /// A callout kind by its name; a name that is no slug is a note, and says so.
    fn callout_kind(&mut self, name: &str, span: Span) -> CalloutKind {
        if let Ok(slug) = Slug::parse(name) {
            return CalloutKind::Other(slug);
        }
        let announcement = Announcement::CalloutKindUnknown { kind: name.to_owned() };
        self.notices.push(Notice::new(announcement).on(self.subject.clone()).at(span));
        CalloutKind::Note
    }

    /// A block nested past [`DEPTH`]: its text as one paragraph.
    fn too_deep<'a>(&mut self, node: &'a AstNode<'a>, out: &mut Vec<Block>) {
        let text = node.children().map(text_of).collect::<Vec<_>>().join(" ");
        let text = text.trim();
        if !text.is_empty() {
            out.push(Block::Paragraph(vec![Inline::Text(text.to_owned())]));
        }
        self.nested(node);
    }
}

fn code_block(code: &NodeCodeBlock) -> Block {
    let language = code.info.split_whitespace().next().map(str::to_owned);
    if language.as_deref() == Some("math") {
        return Block::Math(code.literal.trim_end_matches('\n').to_owned());
    }
    Block::Code { language, text: code.literal.clone() }
}

const fn alignment(alignment: TableAlignment) -> Alignment {
    match alignment {
        TableAlignment::None => Alignment::None,
        TableAlignment::Left => Alignment::Left,
        TableAlignment::Center => Alignment::Center,
        TableAlignment::Right => Alignment::Right,
    }
}
