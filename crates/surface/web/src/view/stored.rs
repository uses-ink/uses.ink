//! A build's log read back from storage: one line for each event the build wrote, in order,
//! shown as the event's name and its fields as they were stored. A build that has not ended has
//! no log yet, and its page asks again every few seconds until it does.

use std::fmt;

use build::BuildId;
use control::Handle;
use unspliced::{Halt, Render, Sink, html};

use crate::escaped::Escaped;
use crate::route::{Path, Screen};
use crate::shell::{Frame, View};
use crate::style::look;
use crate::view::block::Link;
use crate::view::build::{BuildTitle, Ended, Verdict};

/// One stored event: its name, the rest as stored, and whether it is a problem the build hit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recorded {
    /// The event's name, such as `source-fetched`.
    pub event: String,
    /// Its fields, written out as they were stored.
    pub detail: String,
    /// Whether it says the build was refused or had an error.
    pub bad: bool,
}

/// A build's stored log as a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stored {
    /// Where the page sits.
    pub frame: Frame,
    /// The site's handle.
    pub handle: Handle,
    /// The build.
    pub build: BuildId,
    /// Its log, in order.
    pub lines: Vec<Recorded>,
    /// How it ended; none while it is queued or running.
    pub ended: Option<Ended>,
}

impl Stored {
    /// The log of `build`, which ended as `ended` (or has not, with none).
    pub fn new(handle: Handle, build: BuildId, lines: Vec<Recorded>, ended: Option<Ended>) -> Self {
        Self { frame: Frame::of(&handle, Screen::Deploys), handle, build, lines, ended }
    }
}

impl View for Stored {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        Escaped(BuildTitle(self.build))
    }

    fn refresh(&self) -> bool {
        self.ended.is_none()
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let deploys = Path::of(Screen::Deploys, &self.handle);
        html! {
            p { (Link(&deploys, "All deploys")) }
            @if self.ended.is_none() { p.look::NOTE role="status" { "This build has not ended, so it has no log yet. This page checks again every few seconds." } }
            div.look::LOG role="log" aria-label="Build log" tabindex=0 {
                @if self.lines.is_empty() { div { "The log is empty." } }
                @for line in &self.lines {
                    @if line.bad {
                        div.look::BAD { "x " code.look::PATH { (&line.event) } " " (&line.detail) }
                    } @else {
                        div { code.look::PATH { (&line.event) } " " (&line.detail) }
                    }
                }
            }
            @if let Some(ended) = &self.ended { (Verdict { handle: &self.handle, ended }) }
        }
        .render(out)
    }
}
