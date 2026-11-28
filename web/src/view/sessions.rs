//! The signed-in sessions on the Account screen: when each began and was last used, which one
//! this page is, and a button to end any other. Ending is a plain form post, so it needs no script.

use jiff::Timestamp;
use unspliced::keyword::ButtonType;
use unspliced::{Halt, Lazy, Render, Sink, html};

use crate::route::Path;
use crate::style::look;
use crate::view::block::{Post, Stamp};

/// One signed-in session of the person, as the Account screen lists it. It holds no secret: its id
/// names it in the form that ends it and signs nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    /// Its id, which the end button names.
    pub id: control::Id<control::SessionKind>,
    /// When it began.
    pub created: Timestamp,
    /// When it was last used.
    pub seen: Timestamp,
    /// Whether this page was asked for with it.
    pub current: bool,
}

/// The sessions section of the Account screen.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Section<'a>(pub(crate) &'a [Seen]);

impl Render for Section<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        html! {
            h2 { "Sessions" }
            @if self.0.is_empty() {
                p { "No other session is signed in." }
            } @else {
                ul.look::STACK {
                    @for seen in self.0 { li { (Lazy(move |out: &mut dyn Sink| Self::row(seen, out))) } }
                }
            }
        }
        .render(out)
    }
}

impl Section<'_> {
    fn row(seen: &Seen, out: &mut dyn Sink) -> Result<(), Halt> {
        let to = Path::session(seen.id);
        html! {
            "Began " (Stamp(seen.created)) ", last used " (Stamp(seen.seen))
            @if seen.current {
                strong { " This session." }
            } @else {
                (Post { to: &to, name: "End this session", body: html! {
                    button type=(ButtonType::Submit) { "End this session" }
                } })
            }
        }
        .render(out)
    }
}
