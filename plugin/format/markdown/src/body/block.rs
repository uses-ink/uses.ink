//! Blocks.

use comrak::nodes::{AlertType, AstNode, ListType, NodeAlert, NodeCodeBlock, NodeList, NodeValue, TableAlignment};
use model::{Alignment, Block, CalloutKind, Footnote, Inline, Item, Level, Slug, Span, Spanned, Task};

use super::inline::{has_content, plain};
use super::{Content, DEPTH, Walk, text_of};
use crate::html::{self, Piece, Pieces};

impl Walk<'_> {
    pub(super) fn blocks<'a>(&mut self, node: &'a AstNode<'a>) -> Vec<Block> {
        let mut out = Vec::new();
        if self.depth >= DEPTH {
            self.too_deep(node, &mut out);
            return out;
        }
        self.depth += 1;
        for child in node.children() {
            self.block(child, &mut out);
        }
        self.depth -= 1;
        out
    }

    fn block<'a>(&mut self, node: &'a AstNode<'a>, out: &mut Vec<Block>) {
        let data = node.data();
        let span = self.span(data.sourcepos);
        match &data.value {
            NodeValue::Paragraph => self.paragraph(node, out),
            NodeValue::Heading(heading) => out.push(self.heading(node, heading.level, span)),
            NodeValue::List(list) => out.push(self.list(node, list)),
            NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => out.push(Block::Quote(self.blocks(node))),
            NodeValue::CodeBlock(code) => out.push(code_block(code)),
            NodeValue::HtmlBlock(block) => self.html_block(&block.literal, span, out),
            NodeValue::ThematicBreak(_) => out.push(Block::Rule),
            NodeValue::Table(table) => out.push(self.table(node, &table.alignments)),
            NodeValue::Alert(alert) => out.push(self.callout(node, alert)),
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

    fn callout<'a>(&mut self, node: &'a AstNode<'a>, alert: &NodeAlert) -> Block {
        let kind = match alert.alert_type {
            AlertType::Note => CalloutKind::Note,
            AlertType::Tip => CalloutKind::Tip,
            AlertType::Important => CalloutKind::Important,
            AlertType::Warning => CalloutKind::Warning,
            AlertType::Caution => CalloutKind::Caution,
            // a kind comrak learns later keeps its own name
            other => Slug::from_text(format!("{other:?}")).map_or(CalloutKind::Note, CalloutKind::Other),
        };
        let title = alert.title.as_ref().map(|title| vec![Inline::Text(title.clone())]);
        Block::Callout { kind, title, blocks: self.blocks(node) }
    }

    /// Raw HTML removed: each tag reported, the text between kept as a paragraph.
    fn html_block(&mut self, literal: &str, span: Span, out: &mut Vec<Block>) {
        let placement = self.placement(literal, span);
        let mut text = String::new();
        let mut content = Content::Prose;
        for piece in Pieces::new(literal) {
            if let (Piece::Text(written), Content::Prose) = (&piece, content) {
                text.push_str(written);
            }
            content = content.after(&piece);
            self.strip(&piece, placement);
        }
        let text = html::decode(&text).split_whitespace().collect::<Vec<_>>().join(" ");
        if !text.is_empty() {
            out.push(Block::Paragraph(vec![Inline::Text(text)]));
        }
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
