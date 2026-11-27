//! Inlines: how a comrak inline joins its run, where links and images point, and the text a
//! reader sees.

use comrak::nodes::{AstNode, NodeValue, NodeWikiLink};
use model::{Anchor, Announcement, ExternalUrl, FootnoteName, ImageSource, Inline, LinkTarget, Slug, Span, Visit};

use super::html::Open;
use super::stack::Stack;
use super::{Builder, Literal, Node};
use crate::dialect::{Dialect, Linked, Wikilink};

impl Node for Inline {
    type Open = Open;

    fn read<'a, D: Dialect>(child: &'a AstNode<'a>, builder: &mut Builder<'_, D>, run: &mut Stack<Open>) {
        let data = child.data();
        let inline = match &data.value {
            NodeValue::HtmlInline(raw) => return builder.tags(raw, builder.positions.span(data.sourcepos), run),
            _ if run.is_opaque() => return,
            NodeValue::Text(text) => builder.text(text, builder.positions.span(data.sourcepos)),
            NodeValue::WikiLink(link) => builder.inline(child, link, builder.positions.span(data.sourcepos)),
            NodeValue::BlockId(id) => {
                let span = builder.positions.span(data.sourcepos);
                let Some(anchor) = builder.names.block(&id.name, span, builder.report) else { return };
                Self::Anchor(anchor)
            }
            NodeValue::SoftBreak => Self::SoftBreak,
            NodeValue::LineBreak => Self::Break,
            NodeValue::Code(code) => Self::Code(code.literal.clone()),
            NodeValue::Emph(_) => Self::Emphasis(builder.run(child)),
            NodeValue::Strong(_) => Self::Strong(builder.run(child)),
            NodeValue::Strikethrough(_) => Self::Strike(builder.run(child)),
            NodeValue::Highlight => Self::Highlight(builder.run(child)),
            NodeValue::Superscript => Self::Superscript(builder.run(child)),
            NodeValue::Subscript => Self::Subscript(builder.run(child)),
            NodeValue::Link(link) => Self::Link { target: Written(&link.url).into(), children: builder.label(child) },
            NodeValue::Image(link) => {
                Self::Image { source: Written(&link.url).into(), alt: Plain::of(&builder.run(child)) }
            }
            NodeValue::Math(math) => Self::Math(math.literal.clone()),
            NodeValue::FootnoteReference(reference) => {
                match builder.names.name::<FootnoteName>(&reference.name, None, builder.report) {
                    Some(name) => Self::Footnote(name),
                    None => return,
                }
            }
            _ => {
                for inline in builder.run(child) {
                    run.push(inline);
                }
                return;
            }
        };
        run.push(inline);
    }

    /// The children's text as one run of text.
    fn flatten<'a>(parent: &'a AstNode<'a>) -> Option<Self> {
        let text: String = parent.children().map(|child| Literal(child).to_string()).collect();
        (!text.is_empty()).then_some(Self::Text(text))
    }

    /// Text joins the text before it, so the document holds one text per run; empty text is
    /// nothing.
    fn join(run: &mut Vec<Self>, inline: Self) {
        let Self::Text(text) = inline else { return run.push(inline) };
        match run.last_mut() {
            _ if text.is_empty() => {}
            Some(Self::Text(last)) => last.push_str(&text),
            _ => run.push(Self::Text(text)),
        }
    }

    /// More than whitespace and line breaks.
    fn shows(run: &[Self]) -> bool {
        run.iter().any(|inline| match inline {
            Self::Text(text) => !text.trim().is_empty(),
            Self::Break | Self::SoftBreak => false,
            _ => true,
        })
    }
}

impl<D: Dialect> Builder<'_, D> {
    /// Text of the body at `span`, which the dialect reads unless it is a link's.
    fn text(&mut self, text: &str, span: Span) -> Inline {
        if self.linked == 0 {
            self.dialect.text(text, span);
        }
        Inline::Text(text.to_owned())
    }

    /// A wikilink among other inlines: an embed of a page is a link here, and that is said.
    fn inline<'a>(&mut self, node: &'a AstNode<'a>, link: &NodeWikiLink, span: Span) -> Inline {
        let linked = self.wiki(node, link, span);
        if let Some(written) = linked.embed {
            self.report.announce(Announcement::EmbedInlineLinked { written }, span);
        }
        linked.inline
    }

    /// The text of the link `node`, read as inlines.
    fn label<'a>(&mut self, node: &'a AstNode<'a>) -> Vec<Inline> {
        self.linked += 1;
        let label = self.run(node);
        self.linked -= 1;
        label
    }

    /// What the wikilink `node` becomes in the dialect, its children read as its label; what
    /// the dialect says of it is announced at `span`.
    pub(super) fn wiki<'a>(&mut self, node: &'a AstNode<'a>, link: &NodeWikiLink, span: Span) -> Linked {
        let label = self.label(node);
        let linked = self.dialect.wikilink(Wikilink { target: &link.url, label, embedded: link.embed, span });
        if let Some(said) = &linked.said {
            self.report.announce(said.clone(), span);
        }
        linked
    }
}

/// A link's or an image's destination as written, read as far as one file can tell. Nothing
/// is resolved: a path stays as written for the engine.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Written<'a>(pub(crate) &'a str);

impl From<Written<'_>> for LinkTarget {
    /// `http(s):` and `mailto:` are external, `#anchor` alone is an anchor when it is a slug,
    /// anything else is unresolved.
    fn from(Written(url): Written<'_>) -> Self {
        if let Ok(external) = ExternalUrl::parse(url) {
            return Self::External(external);
        }
        if let Some(slug) = url.strip_prefix('#').and_then(|anchor| Slug::parse(anchor).ok()) {
            return Self::Anchor(Anchor::new(slug));
        }
        Self::Unresolved { written: url.to_owned() }
    }
}

impl From<Written<'_>> for ImageSource {
    /// External, or unresolved.
    fn from(Written(url): Written<'_>) -> Self {
        match ExternalUrl::parse(url) {
            Ok(external) => Self::External(external),
            Err(_) => Self::Unresolved { written: url.to_owned() },
        }
    }
}

/// The text of inlines as a reader sees it: code, math and keys as written, an image as its
/// alt text, breaks as spaces.
#[derive(Debug, Default)]
pub struct Plain(String);

impl Plain {
    /// The plain text of `inlines`.
    pub fn of(inlines: &[Inline]) -> String {
        let mut plain = Self::default();
        plain.inlines(inlines);
        plain.0
    }
}

impl Visit for Plain {
    fn inline(&mut self, inline: &Inline) {
        match inline {
            Inline::Code(text) | Inline::Math(text) | Inline::Keyboard(text) => self.0.push_str(text),
            Inline::Break | Inline::SoftBreak => self.0.push(' '),
            _ => inline.walk(self),
        }
    }

    fn image(&mut self, _: &ImageSource, alt: &str) {
        self.0.push_str(alt);
    }

    fn text(&mut self, text: &str) {
        self.0.push_str(text);
    }
}
