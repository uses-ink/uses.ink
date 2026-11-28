//! One decision's ladder replayed rung by rung: what each witness said, including the ones that
//! found nothing, and which testimony was chosen (`ink explain --why`, decision 0042).

use std::fmt;

use control::Handle;
use engine::{Standing, Trace, Witnessed};
use model::{AnyCode, AnyDecision, Subject};
use unspliced::{Halt, Render, Sink, html};

use crate::escaped::Escaped;
use crate::route::{Path, Screen};
use crate::shell::{Frame, View};
use crate::style::look;
use crate::view::block::{Empty, Link};

/// One testimony a witness gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heard {
    /// The value and its evidence, as the decision it would have been.
    pub testimony: AnyDecision,
    /// Whether it was a value or only a bound.
    pub standing: Standing,
    /// Whether it is the decision.
    pub chosen: bool,
}

/// One witness of a ladder and what it said; nothing when it found nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rung {
    /// The witness.
    pub witness: Witnessed,
    /// What it said, in order.
    pub heard: Vec<Heard>,
}

/// What the replay screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Why {
    /// Where the page sits.
    pub frame: Frame,
    /// The site's handle.
    pub handle: Handle,
    /// The code decided.
    pub code: AnyCode,
    /// What it was decided about.
    pub subject: Subject,
    /// Every witness of the ladder, first first.
    pub rungs: Vec<Rung>,
}

impl Why {
    /// The replay of `trace` for the site `handle`.
    pub fn traced(handle: Handle, trace: &Trace) -> Self {
        let rungs = trace
            .rungs
            .iter()
            .map(|rung| Rung {
                witness: rung.witness,
                heard: rung
                    .heard
                    .iter()
                    .map(|heard| Heard {
                        testimony: heard.testimony.clone(),
                        standing: heard.standing,
                        chosen: heard.chosen,
                    })
                    .collect(),
            })
            .collect();
        Self {
            frame: Frame::of(&handle, Screen::Understood),
            handle,
            code: trace.code,
            subject: trace.subject.clone(),
            rungs,
        }
    }
}

impl View for Why {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        Escaped(self.code)
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let back = Path::of(Screen::Understood, &self.handle);
        html! {
            p.look::LEDE { "How " code.look::PATH { (self.code.name()) } " on " code.look::PATH { (Escaped(&self.subject)) } " was decided: each witness in order, first first. The first one that said a value wins." }
            @if self.rungs.is_empty() {
                (Empty { title: "No ladder to show", what: "The build kept no trace of this decision. Run `ink explain --why` on your own copy of the source to replay it." })
            } @else {
                ol.look::LADDER {
                    @for rung in &self.rungs {
                        li {
                            strong { (Escaped(rung.witness)) }
                            @if rung.heard.is_empty() { " found nothing" } @else {
                                ul {
                                    @for heard in &rung.heard {
                                        li { (Testified(heard)) }
                                    }
                                }
                            }
                        }
                    }
                }
                p { "To pin it: " code.look::PATH { (Escaped(diag::PinCommand(&self.code, &self.subject))) } }
            }
            p { (Link(&back, "Back to What we understood")) }
        }
        .render(out)
    }
}

/// One testimony: its value, its evidence and how it stood.
#[derive(Clone, Copy, Debug)]
struct Testified<'a>(&'a Heard);

impl Render for Testified<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let heard = self.0;
        let standing = match (heard.chosen, heard.standing) {
            (true, _) => "chosen",
            (false, Standing::Bound) => "a bound, never chosen",
            (false, _) => "outranked",
        };
        let said = html! {
            (Escaped(diag::Value(&heard.testimony))) ", " (Escaped(diag::Because(heard.testimony.evidence()))) ", " (standing)
        };
        if heard.chosen { html! { span.look::CHOSEN { (said) } }.render(out) } else { said.render(out) }
    }
}
