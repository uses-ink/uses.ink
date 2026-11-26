//! Inlines, links and images.

use comrak::nodes::{AstNode, NodeValue};
use model::{Anchor, Diagnostic, ExternalUrl, ImageSource, Inline, LinkTarget, Problem, Slug, Span};

use super::raw::Inlines;
use super::{DEPTH, Walk, text_of};
use crate::html::Pieces;

impl Walk<'_> {
    pub(super) fn inlines<'a>(&mut self, node: &'a AstNode<'a>) -> Vec<Inline> {
        if self.depth >= DEPTH {
            let mut out = Vec::new();
            let text = node.children().map(text_of).collect::<String>();
            push_text(&mut out, &text);
            self.nested(node);
            return out;
        }
        self.depth += 1;
        let mut inlines = Inlines::default();
        for child in node.children() {
            self.inline(child, &mut inlines);
        }
        self.depth -= 1;
        self.finish(inlines)
    }

    /// One inline into `inlines`, where raw HTML before it may have left elements open.
    fn inline<'a>(&mut self, node: &'a AstNode<'a>, inlines: &mut Inlines) {
        let data = node.data();
        let inline = match &data.value {
            NodeValue::HtmlInline(raw) => return self.html_inline(raw, self.span(data.sourcepos), inlines),
            _ if inlines.is_opaque() => return,
            NodeValue::Text(text) => return inlines.push_text(text),
            NodeValue::SoftBreak => Inline::SoftBreak,
            NodeValue::LineBreak => Inline::Break,
            NodeValue::Code(code) => Inline::Code(code.literal.clone()),
            NodeValue::Emph(_) => Inline::Emphasis(self.inlines(node)),
            NodeValue::Strong(_) => Inline::Strong(self.inlines(node)),
            NodeValue::Strikethrough(_) => Inline::Strike(self.inlines(node)),
            NodeValue::Highlight => Inline::Highlight(self.inlines(node)),
            NodeValue::Superscript => Inline::Superscript(self.inlines(node)),
            NodeValue::Subscript => Inline::Subscript(self.inlines(node)),
            NodeValue::Link(link) => Inline::Link { target: target(&link.url), children: self.inlines(node) },
            NodeValue::Image(link) => Inline::Image { source: source(&link.url), alt: plain(&self.inlines(node)) },
            NodeValue::Math(math) => Inline::Math(math.literal.clone()),
            NodeValue::FootnoteReference(reference) => match self.footnote_name(&reference.name, None) {
                Some(name) => Inline::Footnote(name),
                None => return,
            },
            _ => {
                for inline in self.inlines(node) {
                    inlines.push(inline);
                }
                return;
            }
        };
        inlines.push(inline);
    }

    /// One inline tag, paired with its siblings.
    fn html_inline(&mut self, raw: &str, span: Span, inlines: &mut Inlines) {
        let placement = self.placement(raw, span);
        for piece in Pieces::new(raw) {
            self.piece(inlines, &piece, placement.at(&piece));
        }
    }

    /// Reports content nested past [`DEPTH`].
    pub(super) fn nested<'a>(&mut self, node: &'a AstNode<'a>) {
        let span = self.span(node.data().sourcepos);
        let problem = Problem::PageNestingDeep { limit: DEPTH as u64 };
        self.diagnostics.push(Diagnostic::new(problem).on(self.subject.clone()).at(span));
    }
}

/// Where a link goes, as far as one file can tell.
pub(super) fn target(url: &str) -> LinkTarget {
    if let Ok(external) = ExternalUrl::parse(url) {
        return LinkTarget::External(external);
    }
    if let Some(slug) = url.strip_prefix('#').and_then(|anchor| Slug::parse(anchor).ok()) {
        return LinkTarget::Anchor(Anchor::new(slug));
    }
    LinkTarget::Unresolved { written: url.to_owned() }
}

/// Where an image comes from, as far as one file can tell.
pub(super) fn source(url: &str) -> ImageSource {
    match ExternalUrl::parse(url) {
        Ok(external) => ImageSource::External(external),
        Err(_) => ImageSource::Unresolved { written: url.to_owned() },
    }
}

/// Text appended to the text before it, if any, so the document holds one run per run.
pub(super) fn push_text(out: &mut Vec<Inline>, text: &str) {
    if text.is_empty() {
        return;
    }
    match out.last_mut() {
        Some(Inline::Text(last)) => last.push_str(text),
        _ => out.push(Inline::Text(text.to_owned())),
    }
}

/// An inline appended, text joined to the text before it.
pub(super) fn push(out: &mut Vec<Inline>, inline: Inline) {
    match inline {
        Inline::Text(text) => push_text(out, &text),
        other => out.push(other),
    }
}

/// The text of inlines as a reader sees it, breaks as spaces.
pub(super) fn plain(inlines: &[Inline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text(t) | Inline::Code(t) | Inline::Math(t) | Inline::Keyboard(t) => text.push_str(t),
            Inline::Emphasis(children)
            | Inline::Strong(children)
            | Inline::Strike(children)
            | Inline::Highlight(children)
            | Inline::Superscript(children)
            | Inline::Subscript(children)
            | Inline::Link { children, .. } => text.push_str(&plain(children)),
            Inline::Image { alt, .. } => text.push_str(alt),
            Inline::Break | Inline::SoftBreak => text.push(' '),
            _ => {}
        }
    }
    text
}

/// Whether inlines show anything: more than whitespace and line breaks.
pub(super) fn has_content(inlines: &[Inline]) -> bool {
    inlines.iter().any(|inline| match inline {
        Inline::Text(text) => !text.trim().is_empty(),
        Inline::Break | Inline::SoftBreak => false,
        _ => true,
    })
}
