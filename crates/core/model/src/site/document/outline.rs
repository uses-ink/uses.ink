//! The headings of a page, for an outline beside it.

use super::{Anchor, Block, Document, Inline, Level, Visit};

/// One heading of a document: what an outline lists.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Heading {
    /// 1 to 6.
    pub level: Level,
    /// The place a link to it points at.
    pub anchor: Anchor,
    /// Its text, without marks.
    pub text: String,
}

/// The text of some inlines, as a reader reads it: code and math as written, a break as a space.
#[derive(Debug, Default)]
struct Plain(String);

impl Visit for Plain {
    fn text(&mut self, text: &str) {
        self.0.push_str(text);
    }

    fn inline(&mut self, inline: &Inline) {
        match inline {
            Inline::Code(text) | Inline::Math(text) | Inline::Keyboard(text) => self.0.push_str(text),
            Inline::Break | Inline::SoftBreak => self.0.push(' '),
            other => other.walk(self),
        }
    }
}

/// Every heading of the body, in order.
#[derive(Debug, Default)]
struct Found(Vec<Heading>);

impl Visit for Found {
    fn block(&mut self, block: &Block) {
        match block {
            Block::Heading { level, anchor, inlines } => {
                let mut plain = Plain::default();
                plain.inlines(inlines);
                self.0.push(Heading { level: *level, anchor: anchor.clone(), text: plain.0 });
            }
            // an embedded page's headings are that page's outline
            Block::Embed { .. } => {}
            other => other.walk(self),
        }
    }

    fn footnote(&mut self, _footnote: &super::Footnote) {}
}

impl Document {
    /// The headings of the body in reading order, a footnote's and an embedded page's left out.
    pub fn outline(&self) -> Vec<Heading> {
        let mut found = Found::default();
        found.blocks(&self.blocks);
        found.0
    }
}
