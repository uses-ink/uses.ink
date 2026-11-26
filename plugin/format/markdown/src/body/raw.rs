//! Raw HTML in a run of inlines: each tag arrives as its own sibling, so opening and closing tags
//! are paired here, among siblings. A paired element on the allowlist becomes its node; every
//! other tag, and an allowed one left unclosed or closed out of order, is removed with
//! `html.element.stripped` and its text kept.

use model::{Diagnostic, Inline, LinkTarget, Problem, Span};

use super::Walk;
use super::inline::{has_content, plain, push, push_text, source, target};
use crate::html::{self, Element, Piece, Tag};

/// Inlines gathered while tags are open.
#[derive(Debug, Default)]
pub(super) struct Inlines {
    done: Vec<Inline>,
    open: Vec<Open>,
    /// Inside `<script>` or `<style>`, whose content is code, not prose: dropped until it closes.
    opaque: bool,
    /// Tags whose opening was reported as removed, so their closing tag says nothing more.
    reported: Vec<String>,
}

/// An element opened and not yet closed.
#[derive(Debug)]
struct Open {
    wrap: Wrap,
    name: String,
    /// Where its opening tag is.
    span: Span,
    children: Vec<Inline>,
}

/// The node an open element becomes once closed.
#[derive(Debug)]
enum Wrap {
    Keyboard,
    Superscript,
    Subscript,
    Highlight,
    Strike,
    Link(LinkTarget),
}

impl Wrap {
    fn around(self, children: Vec<Inline>) -> Inline {
        match self {
            Self::Keyboard => Inline::Keyboard(plain(&children)),
            Self::Superscript => Inline::Superscript(children),
            Self::Subscript => Inline::Subscript(children),
            Self::Highlight => Inline::Highlight(children),
            Self::Strike => Inline::Strike(children),
            Self::Link(target) => Inline::Link { target, children },
        }
    }
}

impl Inlines {
    /// Where the next inline goes: into the innermost open element, or the run itself.
    fn children(&mut self) -> &mut Vec<Inline> {
        match self.open.last_mut() {
            Some(open) => &mut open.children,
            None => &mut self.done,
        }
    }

    /// Adds an inline, unless inside code.
    pub(super) fn push(&mut self, inline: Inline) {
        if !self.opaque {
            push(self.children(), inline);
        }
    }

    /// Adds text, joined to the text before it, unless inside code.
    pub(super) fn push_text(&mut self, text: &str) {
        if !self.opaque {
            push_text(self.children(), text);
        }
    }

    /// Whether what comes now is the code inside `<script>` or `<style>`.
    pub(super) const fn is_opaque(&self) -> bool {
        self.opaque
    }

    /// Whether nothing that shows was gathered and nothing is open.
    pub(super) fn is_blank(&self) -> bool {
        self.open.is_empty() && !has_content(&self.done)
    }
}

/// Text with each run of HTML whitespace as one space, as a browser shows it.
fn collapse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for c in text.chars() {
        if c.is_ascii_whitespace() {
            space = true;
        } else {
            if space {
                out.push(' ');
                space = false;
            }
            out.push(c);
        }
    }
    if space {
        out.push(' ');
    }
    out
}

impl Walk<'_> {
    /// One piece of raw HTML among inlines, at `span` in the file.
    pub(super) fn piece(&mut self, inlines: &mut Inlines, piece: &Piece<'_>, span: Span) {
        if inlines.opaque {
            inlines.opaque = !piece.closes_opaque();
            return;
        }
        if piece.opens_opaque() {
            inlines.opaque = true;
        }
        match piece {
            Piece::Comment => {}
            Piece::Text(text) => inlines.push_text(&collapse(&html::decode(text))),
            Piece::Open(tag) => self.open(inlines, tag, span),
            Piece::Close(tag) => self.close(inlines, tag, span),
        }
    }

    fn open(&mut self, inlines: &mut Inlines, tag: &Tag<'_>, span: Span) {
        let wrap = match tag.name.parse::<Element>() {
            Ok(Element::Break) => return inlines.push(Inline::Break),
            Ok(Element::Image) => return self.image(inlines, tag, span),
            Ok(Element::Keyboard) => Wrap::Keyboard,
            Ok(Element::Superscript) => Wrap::Superscript,
            Ok(Element::Subscript) => Wrap::Subscript,
            Ok(Element::Highlight) => Wrap::Highlight,
            Ok(Element::Strike) => Wrap::Strike,
            Ok(Element::Link) => match tag.attribute("href") {
                Some(href) => Wrap::Link(target(&href)),
                None => return self.refuse(inlines, tag, span),
            },
            Ok(Element::Rule | Element::Details | Element::Summary) | Err(_) => {
                return self.refuse(inlines, tag, span);
            }
        };
        inlines.open.push(Open { wrap, name: tag.name.clone(), span, children: Vec::new() });
    }

    /// `<img src alt>` as an image; every other attribute is dropped, since none is ever written
    /// out.
    fn image(&mut self, inlines: &mut Inlines, tag: &Tag<'_>, span: Span) {
        let Some(src) = tag.attribute("src") else { return self.refuse(inlines, tag, span) };
        // not a fallback: no `alt` is no alt text, as `![](src)` has none
        let alt = tag.attribute("alt").unwrap_or_default();
        inlines.push(Inline::Image { source: source(&src), alt });
    }

    fn close(&mut self, inlines: &mut Inlines, tag: &Tag<'_>, span: Span) {
        if let Some(at) = inlines.open.iter().rposition(|open| open.name == tag.name) {
            while inlines.open.len() > at + 1 {
                self.unclosed(inlines);
            }
            if let Some(open) = inlines.open.pop() {
                inlines.push(open.wrap.around(open.children));
            }
        } else if let Some(at) = inlines.reported.iter().position(|name| name == &tag.name) {
            inlines.reported.remove(at);
        } else if tag.name.parse::<Element>().is_ok() {
            // closes nothing: a stray end of an element that is otherwise kept
            self.stripped(&tag.name, span);
        }
        // the closing tag of any other element belongs to an opening tag already reported
    }

    /// The innermost open element, never closed: removed, its content kept in its place.
    fn unclosed(&mut self, inlines: &mut Inlines) {
        let Some(open) = inlines.open.pop() else { return };
        self.stripped(&open.name, open.span);
        inlines.reported.push(open.name);
        for child in open.children {
            inlines.push(child);
        }
    }

    /// An opening tag removed.
    fn refuse(&mut self, inlines: &mut Inlines, tag: &Tag<'_>, span: Span) {
        self.stripped(&tag.name, span);
        if tag.name.parse::<Element>().is_ok() {
            inlines.reported.push(tag.name.clone());
        }
    }

    /// The run, with every element still open removed.
    pub(super) fn finish(&mut self, mut inlines: Inlines) -> Vec<Inline> {
        while !inlines.open.is_empty() {
            self.unclosed(&mut inlines);
        }
        inlines.done
    }

    /// Reports the element `name` at `span` as removed.
    pub(super) fn stripped(&mut self, name: &str, span: Span) {
        let problem = Problem::HtmlElementStripped { element: name.to_owned() };
        self.diagnostics.push(Diagnostic::new(problem).on(self.subject.clone()).at(span));
    }
}
