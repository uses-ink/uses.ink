//! Inlines, links and images.

use comrak::nodes::{AstNode, NodeValue};
use model::{Anchor, Diagnostic, ExternalUrl, ImageSource, Inline, LinkTarget, Problem, Slug, Span};

use super::{Content, DEPTH, Walk, text_of};
use crate::html::Pieces;

impl Walk<'_> {
    pub(super) fn inlines<'a>(&mut self, node: &'a AstNode<'a>) -> Vec<Inline> {
        let mut out = Vec::new();
        if self.depth >= DEPTH {
            let text = node.children().map(text_of).collect::<String>();
            push_text(&mut out, &text);
            self.nested(node);
            return out;
        }
        self.depth += 1;
        let mut content = Content::Prose;
        for child in node.children() {
            content = self.inline(child, &mut out, content);
        }
        self.depth -= 1;
        out
    }

    /// One inline into `out`, given what text is here; returns what text is after it, which
    /// raw HTML can change.
    fn inline<'a>(&mut self, node: &'a AstNode<'a>, out: &mut Vec<Inline>, content: Content) -> Content {
        let data = node.data();
        match &data.value {
            NodeValue::Text(text) if content == Content::Prose => push_text(out, text),
            NodeValue::Text(_) => {}
            NodeValue::SoftBreak => out.push(Inline::SoftBreak),
            NodeValue::LineBreak => out.push(Inline::Break),
            NodeValue::Code(code) => out.push(Inline::Code(code.literal.clone())),
            NodeValue::Emph(_) => out.push(Inline::Emphasis(self.inlines(node))),
            NodeValue::Strong(_) => out.push(Inline::Strong(self.inlines(node))),
            NodeValue::Strikethrough(_) => out.push(Inline::Strike(self.inlines(node))),
            NodeValue::Link(link) => out.push(Inline::Link { target: target(&link.url), children: self.inlines(node) }),
            NodeValue::Image(link) => {
                let alt = plain(&self.inlines(node));
                out.push(Inline::Image { source: source(&link.url), alt });
            }
            NodeValue::Math(math) => out.push(Inline::Math(math.literal.clone())),
            NodeValue::FootnoteReference(reference) => {
                if let Some(name) = self.footnote_name(&reference.name, None) {
                    out.push(Inline::Footnote(name));
                }
            }
            NodeValue::HtmlInline(raw) => return self.html_inline(raw, self.span(data.sourcepos), content),
            _ => {
                for inline in self.inlines(node) {
                    push(out, inline);
                }
            }
        }
        content
    }

    /// One inline tag removed and reported.
    fn html_inline(&mut self, raw: &str, span: Span, mut content: Content) -> Content {
        let placement = self.placement(raw, span);
        for piece in Pieces::new(raw) {
            content = content.after(&piece);
            self.strip(&piece, placement);
        }
        content
    }

    /// Reports content nested past [`DEPTH`].
    pub(super) fn nested<'a>(&mut self, node: &'a AstNode<'a>) {
        let span = self.span(node.data().sourcepos);
        let problem = Problem::PageNestingDeep { limit: DEPTH as u64 };
        self.diagnostics.push(Diagnostic::new(problem).on(self.subject.clone()).at(span));
    }
}

/// Where a link goes, as far as one file can tell.
fn target(url: &str) -> LinkTarget {
    if let Ok(external) = ExternalUrl::parse(url) {
        return LinkTarget::External(external);
    }
    if let Some(slug) = url.strip_prefix('#').and_then(|anchor| Slug::parse(anchor).ok()) {
        return LinkTarget::Anchor(Anchor::new(slug));
    }
    LinkTarget::Unresolved { written: url.to_owned() }
}

fn source(url: &str) -> ImageSource {
    match ExternalUrl::parse(url) {
        Ok(external) => ImageSource::External(external),
        Err(_) => ImageSource::Unresolved { written: url.to_owned() },
    }
}

/// Text appended to the text before it, if any, so the document holds one run per run.
fn push_text(out: &mut Vec<Inline>, text: &str) {
    if text.is_empty() {
        return;
    }
    match out.last_mut() {
        Some(Inline::Text(last)) => last.push_str(text),
        _ => out.push(Inline::Text(text.to_owned())),
    }
}

fn push(out: &mut Vec<Inline>, inline: Inline) {
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
            Inline::Text(t) | Inline::Code(t) | Inline::Math(t) => text.push_str(t),
            Inline::Emphasis(children)
            | Inline::Strong(children)
            | Inline::Strike(children)
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
