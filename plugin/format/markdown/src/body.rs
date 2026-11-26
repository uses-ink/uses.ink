//! The body: comrak's tree into a [`Document`].

mod block;
mod inline;

use std::collections::BTreeSet;

use comrak::nodes::{AstNode, NodeValue, Sourcepos};
use model::{
    Anchor, Announcement, Diagnostic, Document, Footnote, FootnoteName, Hash, Notice, Problem, Slug, Span, Spanned,
    Subject,
};

use crate::html::Piece;

/// Deeper than this, a block or inline keeps only its text: a page is not a stack to overflow.
const DEPTH: usize = 64;

/// The anchor of a heading with no letters or digits.
const SECTION: &str = "section";

/// The body read: the document, its first level-1 heading, and what went wrong.
#[derive(Debug)]
pub(crate) struct Body {
    pub(crate) document: Document,
    pub(crate) heading: Option<Spanned<String>>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) notices: Vec<Notice>,
}

impl Body {
    /// Reads the body of `file`, which starts at byte `start`.
    pub(crate) fn read(file: &str, start: usize, subject: Subject) -> Self {
        let text = file.get(start..).unwrap_or_default();
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(&arena, text, &options());
        // a span from 0 is never backwards
        let Ok(whole) = Span::new(0, file.len()) else {
            return Self { document: Document::default(), heading: None, diagnostics: Vec::new(), notices: Vec::new() };
        };
        let mut walk = Walk {
            file,
            whole,
            base: start,
            lines: line_starts(text),
            subject,
            diagnostics: Vec::new(),
            notices: Vec::new(),
            anchors: BTreeSet::new(),
            heading: None,
            depth: 0,
            footnotes: Vec::new(),
        };
        let blocks = walk.blocks(root);
        Self {
            document: Document::new(blocks).with_footnotes(walk.footnotes),
            heading: walk.heading,
            diagnostics: walk.diagnostics,
            notices: walk.notices,
        }
    }
}

fn options() -> comrak::Options<'static> {
    let mut options = comrak::Options::default();
    let extension = &mut options.extension;
    extension.strikethrough = true;
    extension.table = true;
    extension.autolink = true;
    extension.alerts = true;
    extension.math_dollars = true;
    extension.math_dollars_block = true;
    extension.math_code = true;
    extension.footnotes = true;
    extension.tasklist = true;
    options
}

/// Where each line of `text` starts. comrak ends a line at `\n`, `\r\n` or a lone `\r`.
fn line_starts(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut starts = vec![0];
    for (at, &byte) in bytes.iter().enumerate() {
        let ends = byte == b'\n' || (byte == b'\r' && bytes.get(at + 1) != Some(&b'\n'));
        if ends {
            starts.push(at + 1);
        }
    }
    starts
}

/// State while walking comrak's tree.
#[derive(Debug)]
struct Walk<'a> {
    file: &'a str,
    /// The whole file, where a position cannot be placed more precisely.
    whole: Span,
    /// Where the body starts in the file.
    base: usize,
    lines: Vec<usize>,
    subject: Subject,
    diagnostics: Vec<Diagnostic>,
    notices: Vec<Notice>,
    /// Anchors given so far, so a repeated heading gets `-1`, `-2`.
    anchors: BTreeSet<String>,
    heading: Option<Spanned<String>>,
    depth: usize,
    /// Footnote definitions, in the order comrak gives them: by first reference.
    footnotes: Vec<Footnote>,
}

impl Walk<'_> {
    /// A comrak position (1-based lines, 1-based byte columns, inclusive end) as a span of the
    /// file.
    fn span(&self, position: Sourcepos) -> Span {
        let offset = |line: usize, column: usize| {
            let start = self.lines.get(line.saturating_sub(1)).map_or(self.file.len(), |&s| self.base + s);
            (start + column.saturating_sub(1)).min(self.file.len())
        };
        let start = offset(position.start.line, position.start.column);
        let end = (offset(position.end.line, position.end.column) + 1).min(self.file.len()).max(start);
        Span::new(start, end).unwrap_or(self.whole)
    }

    /// The anchor of a heading: its slug, then `-1`, `-2` while taken.
    fn anchor(&mut self, text: &str, span: Span) -> Option<Anchor> {
        let slug = if let Ok(slug) = Slug::from_text(text) {
            slug
        } else {
            let slug = Slug::from_text(SECTION).ok()?;
            let announcement = Announcement::HeadingAnchorGenerated { anchor: self.unique(&slug).to_string() };
            self.notices.push(Notice::new(announcement).on(self.subject.clone()).at(span));
            slug
        };
        let slug = self.unique(&slug);
        self.anchors.insert(slug.as_str().to_owned());
        Some(Anchor::new(slug))
    }

    /// A footnote's name: the slug of what the author wrote. A name with no letters or digits
    /// (`[^*]`) gets `note-` and the start of its hash, the same for its reference and its
    /// definition, and the definition announces it.
    fn footnote_name(&mut self, given: &str, announce: Option<Span>) -> Option<FootnoteName> {
        if let Ok(slug) = Slug::from_text(given) {
            return Some(slug.into());
        }
        let digest = Hash::of(given.as_bytes()).to_string();
        let slug = Slug::parse(format!("note-{}", digest.get(..8)?)).ok()?;
        if let Some(span) = announce {
            let announcement = Announcement::FootnoteNameGenerated { name: slug.to_string(), given: given.to_owned() };
            self.notices.push(Notice::new(announcement).on(self.subject.clone()).at(span));
        }
        Some(slug.into())
    }

    /// `slug`, or the first of `slug-1`, `slug-2` not yet given.
    fn unique(&self, slug: &Slug) -> Slug {
        let mut candidate = slug.clone();
        let mut count = 0_u64;
        while self.anchors.contains(candidate.as_str()) {
            count += 1;
            match Slug::parse(format!("{slug}-{count}")) {
                Ok(next) => candidate = next,
                Err(_) => break,
            }
        }
        candidate
    }

    /// Where raw HTML at `span` is in the file: written there as is, or reshaped by comrak
    /// (container prefixes taken out of its lines).
    fn placement(&self, html: &str, span: Span) -> Placement {
        let exact = self.file.get(span.start()..).is_some_and(|rest| rest.starts_with(html));
        if exact { Placement::Exact(span) } else { Placement::Within(span) }
    }

    /// Reports a tag of raw HTML as removed, at the tag when its place is exact.
    fn strip(&mut self, piece: &Piece<'_>, placement: Placement) {
        let Piece::Open { name, start, end } = piece else { return };
        let span = match placement {
            Placement::Exact(span) => Span::new(span.start() + start, span.start() + end).unwrap_or(span),
            Placement::Within(span) => span,
        };
        let problem = Problem::HtmlElementStripped { element: name.clone() };
        self.diagnostics.push(Diagnostic::new(problem).on(self.subject.clone()).at(span));
    }
}

/// Where a piece of raw HTML is in the file.
#[derive(Clone, Copy, Debug)]
enum Placement {
    /// The HTML is the file's text from this span's start, so a tag's offset in it is exact.
    Exact(Span),
    /// The HTML is somewhere in this span.
    Within(Span),
}

/// What text between raw HTML tags is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Content {
    /// Prose, kept.
    Prose,
    /// Inside `<script>` or `<style>`: code, dropped.
    Code,
}

impl Content {
    /// The content after `piece`.
    fn after(self, piece: &Piece<'_>) -> Self {
        if piece.opens_opaque() {
            Self::Code
        } else if piece.closes_opaque() {
            Self::Prose
        } else {
            self
        }
    }
}

/// The text of everything under `node`, without recursion: what a node too deep keeps.
fn text_of<'a>(node: &'a AstNode<'a>) -> String {
    let mut text = String::new();
    for descendant in node.descendants() {
        match &descendant.data().value {
            NodeValue::Text(t) => text.push_str(t),
            NodeValue::Code(code) => text.push_str(&code.literal),
            NodeValue::CodeBlock(code) => text.push_str(&code.literal),
            NodeValue::Math(math) => text.push_str(&math.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => text.push(' '),
            _ => {}
        }
    }
    text
}
