//! The error state of every screen: what happened, its code, what it was caused by, and the fix.

use std::error::Error;
use std::fmt;

use diag::Plain;
use unspliced::{Halt, Render, Sink, html};

use crate::Failure;
use crate::route::Path;
use crate::shell::{Frame, View};
use crate::style::look;
use crate::view::block::Link;

/// A failure shown as a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failed {
    /// Where the page sits: the screen that failed, when it is known.
    pub frame: Frame,
    /// What failed.
    pub failure: Failure,
}

impl Failed {
    /// `failure` on the page described by `frame`.
    pub const fn new(frame: Frame, failure: Failure) -> Self {
        Self { frame, failure }
    }
}

impl View for Failed {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        "That did not work"
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        FailureBlock(&self.failure).render(out)?;
        let account = Path::account();
        html! { p { (Link(&account, "Go to your account")) } }.render(out)
    }
}

/// A failure as a block: code, what happened, each cause, and the fix.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FailureBlock<'a>(pub(crate) &'a Failure);

impl Render for FailureBlock<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let failure = self.0;
        let causes: Vec<String> = std::iter::successors(failure.source(), |cause| (*cause).source())
            .map(|cause| Plain::flat(cause).to_string())
            .collect();
        let (what, status, fix) =
            (Plain::flat(failure).to_string(), failure.status().to_string(), Plain::flat(failure.fix()).to_string());
        html! {
            section.look::FAILURE role="alert" aria-label="Error" {
                h2 { "Error " code { (failure.code().name()) } }
                dl {
                    dt { "What happened" } dd { (&what) }
                    dt { "Status" } dd { (&status) }
                    @if !causes.is_empty() {
                        dt { "Caused by" }
                        dd { ol { @for cause in &causes { li { (cause) } } } }
                    }
                    dt { "Fix" } dd { (&fix) }
                }
            }
        }
        .render(out)
    }
}
