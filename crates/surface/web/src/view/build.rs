//! A build's log: one typed event per line, the same events the terminal and storage hold. A
//! finished build is one page; a running one is a streamed page (`Streamed`), whose last chunk
//! holds the result.

use std::fmt;
use std::time::Duration;

use build::{BuildId, Event, Status};
use control::Handle;
use model::DeployId;
use unspliced::{HERE, Halt, Lazy, Render, Sink, html};

use crate::Failure;
use crate::escaped::Escaped;
use crate::route::{Path, Screen};
use crate::shell::{Frame, View};
use crate::style::look;
use crate::view::block::{Id, Link};
use crate::view::failed::FailureBlock;
use crate::view::word::Word;

/// How a build ended, as the log says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The status.
    pub status: Status,
    /// The deploy that went live, when one did.
    pub deploy: Option<DeployId>,
    /// How long the build took.
    pub wall: Duration,
}

/// How a streamed log ended.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Ended {
    /// The build finished and said how.
    Finished(Outcome),
    /// The log stopped before the build did, or reading it failed.
    Cut(Failure),
}

impl fmt::Display for Word<Status> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self.0 {
            Status::Succeeded => "published",
            Status::Refused => "refused: the site does not build",
            Status::Failed => "failed: the builder gave up",
            _ => "ended in a way this version cannot show",
        })
    }
}

/// One event of the log, as a line.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Line<'a>(pub(crate) &'a Event);

impl Render for Line<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        match self.0 {
            Event::Started { trigger, .. } => html! { div { "build started, set off by " (Escaped(Word(*trigger))) } }.render(out),
            Event::SourceFetched { files, bytes, .. } => {
                html! { div { "source read: " (*files) " files, " (Escaped(*bytes)) } }.render(out)
            }
            Event::Decided(decision) => html! {
                div {
                    "decided " code.look::PATH { (decision.code().name()) } " on " (Escaped(decision.subject())) ": "
                    (Escaped(diag::Value(decision))) ", " (Escaped(diag::Because(decision.evidence())))
                }
            }
            .render(out),
            Event::Diagnosed(diagnostic) | Event::Refused(diagnostic) => {
                let text = diag::Plain::text(diag::Terminal::from(diagnostic)).to_string();
                match diagnostic.severity() {
                    model::Severity::Error => html! { div.look::BAD { "x " (&text) } }.render(out),
                    _ => html! { div { "! " (&text) } }.render(out),
                }
            }
            Event::Noticed(notice) => html! { div { "i " (diag::Plain::text(diag::Terminal::from(notice)).to_string()) } }.render(out),
            Event::Rendered { files, bytes } => html! { div { "rendered " (*files) " files, " (Escaped(*bytes)) } }.render(out),
            Event::Published { deploy, previous, visible_within } => html! {
                div { "live: deploy " (Id(*deploy)) @if let Some(old) = previous { ", replacing " (Id(*old)) } ", visible everywhere within " (visible_within.as_secs()) " s" }
            }
            .render(out),
            Event::Finished { status, wall } => {
                html! { div { "finished, " (Escaped(Word(*status))) ", in " (wall.as_millis()) " ms" } }.render(out)
            }
            _ => html! { div { "an event this version cannot show" } }.render(out),
        }
    }
}

/// The result of a build, for the end of its page.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Verdict<'a> {
    pub(crate) handle: &'a Handle,
    pub(crate) ended: &'a Ended,
}

impl Render for Verdict<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let (site, deploys) = (Path::of(Screen::Site, self.handle), Path::of(Screen::Deploys, self.handle));
        match self.ended {
            Ended::Finished(Outcome { status: Status::Succeeded, deploy, wall }) => html! {
                section.look::CARD.look::FILL_MINT role="status" aria-label="Result" {
                    h2 { "Result: published" }
                    p { @if let Some(deploy) = deploy { "Deploy " (Id(*deploy)) " is live. " } "The build took " (wall.as_millis()) " ms." }
                    p { (Link(&site, "Back to the site")) " or " (Link(&deploys, "all deploys")) "." }
                }
            }
            .render(out),
            Ended::Finished(Outcome { status, .. }) => html! {
                section.look::FAILURE role="alert" aria-label="Result" {
                    h2 { "Result: " (Escaped(Word(*status))) }
                    p { "Nothing was published; the site still serves what it served before. The lines marked x above say why." }
                    p { strong { "Fix: " } "change what the log names in your source, then push again or press Build now on " (Link(&site, "the site")) "." }
                }
            }
            .render(out),
            Ended::Cut(failure) => FailureBlock(failure).render(out),
        }
    }
}

/// A finished build's log as one page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Build {
    /// Where the page sits.
    pub frame: Frame,
    /// The site's handle.
    pub handle: Handle,
    /// The build.
    pub build: BuildId,
    /// Its log, in order.
    pub events: Vec<Event>,
    /// How it ended.
    pub ended: Ended,
}

impl Build {
    /// The log of `build`, which ended as `ended`.
    pub fn new(handle: Handle, build: BuildId, events: Vec<Event>, ended: Ended) -> Self {
        Self { frame: Frame::of(&handle, Screen::Deploys), handle, build, events, ended }
    }
}

impl View for Build {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        Escaped(BuildTitle(self.build))
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let deploys = Path::of(Screen::Deploys, &self.handle);
        html! {
            p { (Link(&deploys, "All deploys")) }
            div.look::LOG role="log" aria-label="Build log" tabindex=0 {
                @if self.events.is_empty() { div { "The log is empty." } }
                @for event in &self.events { (Line(event)) }
            }
            (Verdict { handle: &self.handle, ended: &self.ended })
        }
        .render(out)
    }
}

/// A build's title: the word and the start of its id.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BuildTitle(pub(crate) BuildId);

impl fmt::Display for BuildTitle {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let whole = self.0.to_string();
        write!(out, "Build {}", whole.get(..8).unwrap_or(&whole))
    }
}

/// A running build's page: the head is sent at once, each line as it arrives, then the tail,
/// whose last block is the result. The log's container is open across the chunks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Live {
    /// Where the page sits.
    pub frame: Frame,
    /// The site's handle.
    pub handle: Handle,
    /// The build.
    pub build: BuildId,
    /// How it ended, once it has; the head is made without it.
    pub ended: Option<Ended>,
}

impl Live {
    /// The page of the running `build`.
    pub fn new(handle: Handle, build: BuildId) -> Self {
        Self { frame: Frame::of(&handle, Screen::Deploys), handle, build, ended: None }
    }

    /// The same page, once the build has ended: its tail holds the result.
    #[must_use]
    pub fn ended(&self, ended: Ended) -> Self {
        Self { ended: Some(ended), ..self.clone() }
    }
}

impl View for Live {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        Escaped(BuildTitle(self.build))
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let deploys = Path::of(Screen::Deploys, &self.handle);
        html! {
            p { (Link(&deploys, "All deploys")) }
            @if self.ended.is_none() { p.look::NOTE role="status" { "Building. Each line shows up as the build writes it, and the result comes last." } }
            div.look::LOG role="log" aria-live="polite" aria-label="Build log" tabindex=0 { (HERE) }
            (Lazy(|out: &mut dyn Sink| match &self.ended {
                Some(ended) => Verdict { handle: &self.handle, ended }.render(out),
                None => Ok(()),
            }))
        }
        .render(out)
    }
}
