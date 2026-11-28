//! Paragraphs that are more than text in a page of panels (`layout = "panels"`, decision 0181):
//! a paragraph that is all emphasis is uses.ink speaking, drawn in the red pen, and a paragraph
//! that is one link, after an optional example address in code, is the page's one action, drawn
//! as an address field and the yellow button.

use model::highlight::Highlight;
use model::{Inline, LinkTarget, Visit};
use unspliced::{HERE, Url, html};

use super::Writer;
use crate::notebook::hand;
use crate::tokens::Flow;

/// What a paragraph is, by its inlines.
#[derive(Clone, Copy, Debug)]
pub(super) enum Voice<'a> {
    /// Text.
    Plain,
    /// The pen: the children of the one emphasis the paragraph is.
    Say(&'a [Inline]),
    /// The action: an example address, where the link goes, and the button's text.
    Ask {
        /// The code before the link, shown in the field until something is typed.
        example: Option<&'a str>,
        /// Where the form goes.
        target: &'a LinkTarget,
        /// What the button says.
        label: &'a [Inline],
    },
}

impl<'a> Voice<'a> {
    /// The voice of a paragraph of `inlines`.
    pub(super) fn of(inlines: &'a [Inline]) -> Self {
        match inlines {
            [Inline::Emphasis(children)] => Self::Say(children),
            [Inline::Link { target, children }] => Self::Ask { example: None, target, label: children },
            [Inline::Code(example), Inline::Text(gap), Inline::Link { target, children }] if gap.trim().is_empty() => {
                Self::Ask { example: Some(example), target, label: children }
            }
            _ => Self::Plain,
        }
    }
}

impl<H: Highlight, R: engine::Sink + ?Sized> Writer<'_, H, R> {
    /// A paragraph of `inlines`: in a page of panels by its voice, else text.
    pub(super) fn paragraph(&mut self, inlines: &[Inline]) {
        let voice = if self.flow == Flow::Panels { Voice::of(inlines) } else { Voice::Plain };
        match voice {
            Voice::Say(children) => self.wrap(html! { p.hand::SAY { (HERE) } }, |p| p.inlines(children)),
            Voice::Ask { example, target, label } => match self.href(target).as_deref().map(Url::checked) {
                Some(Ok(action)) => {
                    let form = html! { form.hand::ASK action=(action) method="get" { (HERE) } };
                    self.wrap(form, |form| {
                        let field = html! { label.hand::FIELD { span.hand::SR { "Repo or vault address" } (HERE) } };
                        form.wrap(field, |field| {
                            field.put(html! { input type="text" name="src" placeholder=(example.unwrap_or("")); });
                        });
                        form.wrap(html! { button.hand::GO type="submit" { (HERE) } }, |button| button.inlines(label));
                    });
                }
                _ => self.wrap(html! { p { (HERE) } }, |p| p.inlines(inlines)),
            },
            Voice::Plain => self.wrap(html! { p { (HERE) } }, |p| p.inlines(inlines)),
        }
    }
}
