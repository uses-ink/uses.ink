//! Raw HTML in the body, through the allowlist.
//!
//! Among inlines, each tag arrives as its own sibling; an allowed element becomes its node once
//! its closing tag pairs on the run's stack ([`Open`]). Among blocks, comrak ends an HTML block
//! at a blank line, so `<details>`, its `<summary>`, the markdown inside and `</details>` arrive
//! as sibling blocks and pair on the blocks' stack ([`Fold`]). `<hr>` alone is a rule. Any other
//! HTML block is its text as a paragraph, with the allowed inline elements kept.

use model::{Block, Inline, Span};

use super::element::{Element, Opening, Wrap};
use super::stack::{Closing, Frame, Stack};
use super::{Builder, Node};
use crate::dialect::Dialect;
use crate::error::Error;
use crate::html::{Collapsed, Decoded, Piece, Pieces, Tag};

/// The tag name of the element a [`Fold`] is.
const DETAILS: &str = "details";

/// The tag name of a details' summary.
const SUMMARY: &str = "summary";

/// An inline element opened and not yet closed.
#[derive(Debug)]
pub(crate) struct Open {
    wrap: Wrap,
    name: String,
    span: Span,
    children: Vec<Inline>,
}

impl Frame for Open {
    type Node = Inline;

    fn name(&self) -> &str {
        &self.name
    }

    fn span(&self) -> Span {
        self.span
    }

    fn children(&mut self) -> &mut Vec<Inline> {
        &mut self.children
    }

    fn close(self) -> Inline {
        self.wrap.around(self.children)
    }

    fn spill(self) -> Vec<Inline> {
        self.children
    }
}

/// A `<details>` opened and not yet closed.
#[derive(Debug)]
pub(crate) struct Fold {
    span: Span,
    summary: Vec<Inline>,
    blocks: Vec<Block>,
}

impl Fold {
    /// Whether a `<summary>` now is its summary: nothing is in it yet.
    fn awaits(&self) -> bool {
        self.summary.is_empty() && self.blocks.is_empty()
    }
}

impl Frame for Fold {
    type Node = Block;

    fn name(&self) -> &str {
        DETAILS
    }

    fn span(&self) -> Span {
        self.span
    }

    fn children(&mut self) -> &mut Vec<Block> {
        &mut self.blocks
    }

    fn close(self) -> Block {
        Block::Details { summary: self.summary, blocks: self.blocks }
    }

    /// Its summary as a paragraph, then its blocks.
    fn spill(self) -> Vec<Block> {
        let summary = Inline::shows(&self.summary).then_some(Block::Paragraph(self.summary));
        summary.into_iter().chain(self.blocks).collect()
    }
}

/// One HTML block being read: the paragraph of its text so far, and a `<summary>` being read.
#[derive(Debug, Default)]
struct Reading {
    text: Stack<Open>,
    /// The summary's inlines and where its opening tag is.
    summary: Option<(Stack<Open>, Span)>,
}

/// What a piece of an HTML block does to the blocks around it.
#[derive(Clone, Copy, Debug)]
enum Step {
    /// `<details>`: the blocks after it are folded into it.
    OpenDetails,
    /// `</details>`: closes the innermost open one.
    CloseDetails,
    /// `<summary>` first in a `<details>`: its handle.
    OpenSummary,
    /// `</summary>`.
    CloseSummary,
    /// `<hr>`.
    Rule,
    /// Anything else: part of the text.
    Inline,
}

impl Reading {
    /// Where inlines go now.
    fn inlines(&mut self) -> &mut Stack<Open> {
        match &mut self.summary {
            Some((summary, _)) => summary,
            None => &mut self.text,
        }
    }

    /// What `piece` does, given what is read so far and the blocks around.
    fn step(&self, piece: &Piece<'_>, blocks: &Stack<Fold>) -> Step {
        if self.text.is_opaque() || self.summary.as_ref().is_some_and(|(summary, _)| summary.is_opaque()) {
            return Step::Inline;
        }
        let Some(tag) = piece.tag() else { return Step::Inline };
        let opens = matches!(piece, Piece::Open(_));
        match (tag.name.parse::<Element>(), opens) {
            (Ok(Element::Details), true) => Step::OpenDetails,
            (Ok(Element::Details), false) => Step::CloseDetails,
            (Ok(Element::Summary), true)
                if self.summary.is_none() && self.text.is_blank() && blocks.top().is_some_and(Fold::awaits) =>
            {
                Step::OpenSummary
            }
            (Ok(Element::Summary), false) if self.summary.is_some() => Step::CloseSummary,
            (Ok(Element::Rule), true) => Step::Rule,
            _ => Step::Inline,
        }
    }

    /// Inlines without the whitespace at their two ends, which HTML does not show.
    fn trim(mut inlines: Vec<Inline>) -> Vec<Inline> {
        if let Some(Inline::Text(first)) = inlines.first_mut() {
            *first = first.trim_start().to_owned();
        }
        if let Some(Inline::Text(last)) = inlines.last_mut() {
            *last = last.trim_end().to_owned();
        }
        inlines.retain(|inline| !matches!(inline, Inline::Text(text) if text.is_empty()));
        inlines
    }
}

impl<D: Dialect> Builder<'_, D> {
    /// One run of inline raw HTML, its tags paired with the siblings around it.
    pub(super) fn tags(&mut self, raw: &str, span: Span, run: &mut Stack<Open>) {
        let placement = self.positions.placement(raw, span);
        for piece in Pieces::new(raw) {
            self.piece(run, &piece, placement.at(&piece));
        }
    }

    /// One HTML block into `blocks`.
    pub(super) fn html(&mut self, literal: &str, span: Span, blocks: &mut Stack<Fold>) {
        let placement = self.positions.placement(literal, span);
        let mut reading = Reading::default();
        for piece in Pieces::new(literal) {
            let at = placement.at(&piece);
            match reading.step(&piece, blocks) {
                Step::OpenDetails => {
                    self.paragraph_end(&mut reading, blocks);
                    blocks.open(Fold { span: at, summary: Vec::new(), blocks: Vec::new() });
                }
                Step::CloseDetails => {
                    self.paragraph_end(&mut reading, blocks);
                    if blocks.close(DETAILS, self.report) != Closing::Paired {
                        self.strip(DETAILS, at);
                    }
                }
                Step::OpenSummary => reading.summary = Some((Stack::default(), at)),
                Step::CloseSummary => {
                    if let (Some((summary, _)), Some(fold)) = (reading.summary.take(), blocks.top_mut()) {
                        fold.summary = summary.finish(self.report);
                    }
                }
                Step::Rule => {
                    self.paragraph_end(&mut reading, blocks);
                    blocks.push(Block::Rule);
                }
                Step::Inline => self.piece(reading.inlines(), &piece, at),
            }
        }
        self.paragraph_end(&mut reading, blocks);
    }

    /// Ends the text read so far as a paragraph; a `<summary>` still open is removed and its
    /// text starts the paragraph. `<br>` alone is a line of its own: a paragraph holding only
    /// the break.
    fn paragraph_end(&mut self, reading: &mut Reading, blocks: &mut Stack<Fold>) {
        let text = std::mem::take(&mut reading.text);
        let mut inlines = Vec::new();
        if let Some((summary, at)) = reading.summary.take() {
            self.strip(SUMMARY, at);
            inlines = summary.finish(self.report);
        }
        for inline in text.finish(self.report) {
            Inline::join(&mut inlines, inline);
        }
        let inlines = Reading::trim(inlines);
        if Inline::shows(&inlines) || inlines.iter().any(|inline| matches!(inline, Inline::Break)) {
            blocks.push(Block::Paragraph(inlines));
        }
    }

    /// One piece of raw HTML among inlines, at `span` in the file.
    fn piece(&mut self, run: &mut Stack<Open>, piece: &Piece<'_>, span: Span) {
        if run.is_opaque() {
            run.hide(!piece.closes_opaque());
            return;
        }
        if piece.opens_opaque() {
            run.hide(true);
        }
        match piece {
            Piece::Comment => {}
            Piece::Text(text) => run.push(Inline::Text(Collapsed(&Decoded(text).to_string()).to_string())),
            Piece::Open(tag) => self.open(run, tag, span),
            Piece::Close(tag) => {
                let closing = run.close(&tag.name, self.report);
                // a stray end of an element that is otherwise kept; the closing tag of any
                // other element belongs to an opening tag already reported
                if closing == Closing::Unmatched && tag.name.parse::<Element>().is_ok() {
                    self.strip(&tag.name, span);
                }
            }
        }
    }

    /// An opening tag among inlines: a node, an element opened, or a tag removed.
    fn open(&mut self, run: &mut Stack<Open>, tag: &Tag<'_>, span: Span) {
        let Ok(element) = tag.name.parse::<Element>() else { return self.strip(&tag.name, span) };
        match element.inline(tag) {
            Some(Opening::Leaf(inline)) => run.push(inline),
            Some(Opening::Wrap(wrap)) => run.open(Open { wrap, name: tag.name.clone(), span, children: Vec::new() }),
            None => {
                self.strip(&tag.name, span);
                run.refuse(tag.name.as_str());
            }
        }
    }

    /// Reports the element `name` at `span` as removed.
    fn strip(&mut self, name: &str, span: Span) {
        self.report.refuse(Error::Stripped { element: name.to_owned() }, span);
    }
}
