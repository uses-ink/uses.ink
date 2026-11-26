//! Raw HTML blocks. comrak ends an HTML block at a blank line, so `<details>`, its `<summary>`,
//! the markdown inside and `</details>` arrive as sibling blocks; they are paired here, among the
//! blocks of one container. `<hr>` alone is a rule. Any other HTML block is its text as a
//! paragraph, with the allowed inline elements kept and every other tag removed.

use model::{Block, Inline, Span};

use super::Walk;
use super::inline::{has_content, push};
use super::raw::Inlines;
use crate::html::{Element, Piece, Pieces};

/// The blocks of one container, with the `<details>` still open in it.
#[derive(Debug, Default)]
pub(super) struct Nest {
    out: Vec<Block>,
    open: Vec<Fold>,
}

/// A `<details>` opened and not yet closed.
#[derive(Debug)]
struct Fold {
    /// Where its opening tag is.
    span: Span,
    summary: Vec<Inline>,
    blocks: Vec<Block>,
}

impl Nest {
    /// Where the next block goes: into the innermost open `<details>`, or the container.
    pub(super) fn target(&mut self) -> &mut Vec<Block> {
        match self.open.last_mut() {
            Some(fold) => &mut fold.blocks,
            None => &mut self.out,
        }
    }

    /// Whether a `<summary>` now is the summary of the innermost `<details>`: nothing is in it
    /// yet.
    fn awaits_summary(&self) -> bool {
        self.open.last().is_some_and(|fold| fold.summary.is_empty() && fold.blocks.is_empty())
    }
}

/// One HTML block being read: the paragraph of its text so far, and a `<summary>` being read.
#[derive(Debug, Default)]
struct Reading {
    text: Inlines,
    /// The summary's inlines and where its opening tag is.
    summary: Option<(Inlines, Span)>,
}

impl Reading {
    /// Where inlines go now.
    fn inlines(&mut self) -> &mut Inlines {
        match &mut self.summary {
            Some((summary, _)) => summary,
            None => &mut self.text,
        }
    }
}

/// What a piece of an HTML block does to the blocks around it.
#[derive(Clone, Copy, Debug)]
enum Step {
    OpenDetails,
    CloseDetails,
    OpenSummary,
    CloseSummary,
    Rule,
    /// Anything else: part of the text.
    Inline,
}

impl Walk<'_> {
    /// One HTML block into `nest`.
    pub(super) fn html_block(&mut self, literal: &str, span: Span, nest: &mut Nest) {
        let placement = self.placement(literal, span);
        let mut reading = Reading::default();
        for piece in Pieces::new(literal) {
            let at = placement.at(&piece);
            match step(&piece, &reading, nest) {
                Step::OpenDetails => {
                    self.end_text(&mut reading, nest);
                    nest.open.push(Fold { span: at, summary: Vec::new(), blocks: Vec::new() });
                }
                Step::CloseDetails => {
                    self.end_text(&mut reading, nest);
                    self.close_details(nest, at);
                }
                Step::OpenSummary => reading.summary = Some((Inlines::default(), at)),
                Step::CloseSummary => {
                    if let (Some((summary, _)), Some(fold)) = (reading.summary.take(), nest.open.last_mut()) {
                        fold.summary = self.finish(summary);
                    }
                }
                Step::Rule => {
                    self.end_text(&mut reading, nest);
                    nest.target().push(Block::Rule);
                }
                Step::Inline => self.piece(reading.inlines(), &piece, at),
            }
        }
        self.end_text(&mut reading, nest);
    }

    /// Ends the text read so far as a paragraph; a `<summary>` still open is removed and its
    /// text joins the paragraph.
    fn end_text(&mut self, reading: &mut Reading, nest: &mut Nest) {
        let text = std::mem::take(&mut reading.text);
        let mut inlines = Vec::new();
        if let Some((summary, at)) = reading.summary.take() {
            self.stripped("summary", at);
            inlines = self.finish(summary);
        }
        for inline in self.finish(text) {
            push(&mut inlines, inline);
        }
        let inlines = trim(inlines);
        // `<br>` alone is a line of its own: a paragraph holding only the break
        if has_content(&inlines) || inlines.iter().any(|inline| matches!(inline, Inline::Break)) {
            nest.target().push(Block::Paragraph(inlines));
        }
    }

    fn close_details(&mut self, nest: &mut Nest, at: Span) {
        match nest.open.pop() {
            Some(fold) => nest.target().push(Block::Details { summary: fold.summary, blocks: fold.blocks }),
            None => self.stripped("details", at),
        }
    }

    /// The container's blocks once all are read: a `<details>` never closed is removed, its
    /// summary and blocks kept in its place.
    pub(super) fn unfold(&mut self, mut nest: Nest) -> Vec<Block> {
        while let Some(fold) = nest.open.pop() {
            self.stripped("details", fold.span);
            let target = nest.target();
            if has_content(&fold.summary) {
                target.push(Block::Paragraph(fold.summary));
            }
            target.extend(fold.blocks);
        }
        nest.out
    }
}

/// What `piece` does, given what is being read.
fn step(piece: &Piece<'_>, reading: &Reading, nest: &Nest) -> Step {
    if reading.text.is_opaque() || reading.summary.as_ref().is_some_and(|(summary, _)| summary.is_opaque()) {
        return Step::Inline;
    }
    let (Piece::Open(tag) | Piece::Close(tag)) = piece else { return Step::Inline };
    let opens = matches!(piece, Piece::Open(_));
    match (tag.name.parse::<Element>(), opens) {
        (Ok(Element::Details), true) => Step::OpenDetails,
        (Ok(Element::Details), false) => Step::CloseDetails,
        (Ok(Element::Summary), true)
            if reading.summary.is_none() && reading.text.is_blank() && nest.awaits_summary() =>
        {
            Step::OpenSummary
        }
        (Ok(Element::Summary), false) if reading.summary.is_some() => Step::CloseSummary,
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
