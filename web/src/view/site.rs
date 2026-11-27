//! The Site screen: is it live, and what changed? State first, then the thing to fix, then
//! history (flow.md, section 8).

use std::fmt;

use control::Handle;
use model::{Diagnostic, ExternalUrl, Observed};
use unspliced::keyword::ButtonType;
use unspliced::{Halt, Lazy, Render, Sink, html};

use crate::escaped::Escaped;
use crate::route::{Path, Screen};
use crate::shell::{Frame, Glyph, View};
use crate::style::look;
use crate::view::block::{Attempt, Empty, Id, Link, Log, Outside, Pill, Post, Problem, Serving, Stamp};
use crate::view::word::Word;

/// What the Site screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Site {
    /// Where the page sits.
    pub frame: Frame,
    /// The site's handle.
    pub handle: Handle,
    /// Its address, `https://<handle>.uses.ink`.
    pub address: ExternalUrl,
    /// What it serves now.
    pub serving: Serving,
    /// The newest builds, newest first; the Site screen lists a handful and Deploys has the rest.
    pub recent: Vec<Attempt>,
    /// What the last build's source could and could not do.
    pub observed: Observed,
    /// The warnings and errors of the last build.
    pub problems: Vec<Diagnostic>,
}

impl Site {
    /// A Site screen for `handle`.
    pub fn new(handle: Handle, address: ExternalUrl, serving: Serving) -> Self {
        Self {
            frame: Frame::of(&handle, Screen::Site),
            handle,
            address,
            serving,
            recent: Vec::new(),
            observed: Observed::default(),
            problems: Vec::new(),
        }
    }
}

impl View for Site {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        &self.handle
    }

    fn refresh(&self) -> bool {
        self.recent.iter().any(Attempt::is_running)
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let address = self.address.to_string();
        html! {
            (Lazy(|out: &mut dyn Sink| self.state(out, &address)))
            (Lazy(|out: &mut dyn Sink| self.notices(out)))
            (Lazy(|out: &mut dyn Sink| self.sections(out)))
        }
        .render(out)
    }
}

impl Site {
    /// The paused banner, the running note and the button that builds now.
    fn notices(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let (build, resume) = (Path::act(&self.handle, "build"), Path::act(&self.handle, "resume"));
        html! {
            @if matches!(self.serving, Serving::Paused(_)) {
                div.look::BANNER role="status" {
                    p { strong { "Publishing is paused." } " A rollback paused it, so new builds are kept as previews and the site stays on the deploy below." }
                    (Post { to: &resume, name: "Resume publishing", body: html! {
                        button.look::MAIN type=(ButtonType::Submit) { "Resume publishing" }
                    } })
                }
            }
            @if self.recent.iter().any(Attempt::is_running) {
                p.look::NOTE role="status" { "A build is running. This page checks again every few seconds until it ends." }
            }
            (Post { to: &build, name: "Build now", body: html! {
                p { button.look::MAIN type=(ButtonType::Submit) { "Build now" } }
                p.look::HINT { "Reads the source again and publishes if anything changed. Nothing is written to your source." }
            } })
        }
        .render(out)
    }

    /// What to fix, what the source can do, and the recent builds.
    fn sections(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let (deploys, understood) =
            (Path::of(Screen::Deploys, &self.handle), Path::of(Screen::Understood, &self.handle));
        html! {
            h2 { "To fix" }
            @if self.problems.is_empty() {
                p { "Nothing to fix. The last build raised no warnings or errors." }
            } @else {
                @for found in &self.problems { (Problem(found)) }
                p { "Why a page looks as it does: " (Link(&understood, "What we understood")) "." }
            }

            h2 { "What the source can do" }
            (Lazy(|out: &mut dyn Sink| self.abilities(out)))

            h2 { "Recent builds" }
            @if self.recent.is_empty() {
                (Empty {
                    title: "No build yet",
                    what: "Nothing has been built for this site. Press Build now, or push your source with `ink push`, and the first deploy shows up here.",
                })
            } @else {
                (Lazy(|out: &mut dyn Sink| self.table(out)))
                p { (Link(&deploys, "All deploys")) }
            }
            p.look::META { "No visitor analytics. Server logs are kept 7 days for abuse handling only." }
        }
        .render(out)
    }

    /// The live address, the deploy served and the latest build, side by side.
    fn state(&self, out: &mut dyn Sink, address: &str) -> Result<(), Halt> {
        html! {
            div.look::GRID {
                section.look::CARD aria-labelledby="live-head" {
                    h2 #live-head { "Live" }
                    @match &self.serving {
                        Serving::Nothing => {
                            p { span.look::STATE.look::NONE { (Glyph("-", "nothing live")) } }
                            p { "Nothing is published at " (Outside(address)) " yet." }
                        },
                        Serving::Live(pointer) => {
                            p { span.look::STATE.look::LIVE { (Glyph("ok", "live")) } }
                            p { "Serving " (Outside(address)) }
                            p.look::META { "deploy " (Id(pointer.deploy)) ", " (pointer.files) " files, " (Escaped(pointer.bytes)) ", since " (Stamp(pointer.since)) }
                        },
                        Serving::Paused(pointer) => {
                            p { span.look::STATE.look::PAUSED { (Glyph("||", "live, paused")) } }
                            p { "Serving " (Outside(address)) }
                            p.look::META { "deploy " (Id(pointer.deploy)) ", " (pointer.files) " files, " (Escaped(pointer.bytes)) ", since " (Stamp(pointer.since)) }
                        },
                    }
                }
                section.look::CARD aria-labelledby="latest-head" {
                    h2 #latest-head { "Latest build" }
                    @if let Some(latest) = self.recent.first() {
                        p { (Pill(latest.state)) }
                        p.look::META { "requested " (Stamp(latest.requested)) ", by " (Escaped(Word(latest.trigger))) }
                        p { (Log(&self.handle, latest.build)) }
                    } @else {
                        p { "No build has run." }
                    }
                }
            }
        }
        .render(out)
    }

    /// What the source can do, and what it lacks.
    fn abilities(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let has = &self.observed.has;
        let lacks = &self.observed.lacks;
        if has.is_empty() && lacks.is_empty() {
            return Empty {
                title: "Not read yet",
                what: "A build tells what its source can do (snapshot, history, watch, private). Build the site once to see it.",
            }
            .render(out);
        }
        html! {
            ul {
                @for ability in has { li { (Glyph("+", "")) (Escaped(Word(*ability))) } }
                @for lack in lacks { li { (Glyph("-", "")) "limited: " (Escaped(Word(lack.ability))) ", so " code.look::PATH { (lack.consequence.name()) } " may be a guess" } }
            }
        }
        .render(out)
    }

    fn table(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        Builds { handle: &self.handle, attempts: &self.recent, caption: "Recent builds" }.render(out)
    }
}

/// A table of builds with the deploy each made.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Builds<'a> {
    pub(crate) handle: &'a Handle,
    pub(crate) attempts: &'a [Attempt],
    pub(crate) caption: &'a str,
}

impl Render for Builds<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        html! {
            div.look::TABLE role="region" aria-label=(self.caption) tabindex=0 {
                table {
                    caption { (self.caption) }
                    thead { tr { th scope="col" { "Requested" } th scope="col" { "Build" } th scope="col" { "Set off by" } th scope="col" { "State" } th scope="col" { "Deploy" } } }
                    tbody {
                        @for attempt in self.attempts {
                            tr {
                                td { (Stamp(attempt.requested)) }
                                td { (Log(self.handle, attempt.build)) }
                                td { (Escaped(Word(attempt.trigger))) }
                                td { (Pill(attempt.state)) }
                                td {
                                    @if let Some(deploy) = &attempt.deploy {
                                        (Id(deploy.id)) " " @if deploy.is_live { strong { "live" } } " " (deploy.files) " files, " (Escaped(deploy.bytes))
                                    } @else { "none" }
                                }
                            }
                        }
                    }
                }
            }
        }
        .render(out)
    }
}
