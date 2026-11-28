//! The Deploys screen: what happened when. Every build with the deploy it made, its log, and a
//! rollback for each deploy that is not live.

use std::fmt;

use control::Handle;
use jiff::Timestamp;
use unspliced::keyword::ButtonType;
use unspliced::{Halt, Lazy, Render, Sink, html};

use crate::escaped::Escaped;
use crate::route::{Path, Screen};
use crate::shell::{Frame, View};
use crate::style::look;
use crate::view::block::{Attempt, Empty, Id, Log, Outside, Pill, Post, Serving, Stamp};
use crate::view::word::Word;

/// A preview of a branch, as the Deploys screen lists it (decision 0091).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
    /// The label, made from the branch name.
    pub label: String,
    /// Where it is served: anyone with the address can open it.
    pub url: String,
    /// How many files it holds.
    pub files: u64,
    /// When it ends.
    pub ends: Timestamp,
}

/// What the Deploys screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deploys {
    /// Where the page sits.
    pub frame: Frame,
    /// The site's handle.
    pub handle: Handle,
    /// What it serves now.
    pub serving: Serving,
    /// Every build, newest first.
    pub history: Vec<Attempt>,
    /// The previews of branches that have not ended, newest first.
    pub previews: Vec<Branch>,
}

impl Deploys {
    /// A Deploys screen for `handle`.
    pub fn new(handle: Handle, serving: Serving, history: Vec<Attempt>) -> Self {
        Self { frame: Frame::of(&handle, Screen::Deploys), handle, serving, history, previews: Vec::new() }
    }

    /// The same screen listing `previews` of branches.
    #[must_use]
    pub fn with_previews(mut self, previews: Vec<Branch>) -> Self {
        self.previews = previews;
        self
    }
}

impl View for Deploys {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        "Deploys"
    }

    fn refresh(&self) -> bool {
        self.history.iter().any(Attempt::is_running)
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let resume = Path::act(&self.handle, "resume");
        html! {
            p.look::LEDE { "Every build and the deploy it made. Open a build for its log." }
            @if matches!(self.serving, Serving::Paused(_)) {
                div.look::BANNER role="status" {
                    p { strong { "Publishing is paused." } " Resume to publish new builds again." }
                    (Post { to: &resume, name: "Resume publishing", body: html! {
                        button.look::MAIN type=(ButtonType::Submit) { "Resume publishing" }
                    } })
                }
            }
            @if self.history.is_empty() {
                (Empty {
                    title: "No deploys yet",
                    what: "A deploy is the hashed result of one build. Press Build now on the Site screen, or push your source with `ink push`, and it is listed here.",
                })
            } @else {
                @if self.refresh() { p.look::NOTE role="status" { "A build is running. This page checks again every few seconds until it ends." } }
                (Lazy(|out: &mut dyn Sink| self.table(out)))
                p.look::META { "Rolling back serves an older deploy and pauses publishing until you resume. Nothing is deleted." }
            }
            @if !self.previews.is_empty() {
                (Lazy(|out: &mut dyn Sink| self.branches(out)))
            }
        }
        .render(out)
    }
}

impl Deploys {
    fn branches(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        html! {
            h2 { "Branch previews" }
            p.look::LEDE { "Public to anyone with the address, not listed anywhere, and removed when they end." }
            div.look::TABLE role="region" aria-label="Branch previews" tabindex=0 {
                table {
                    caption { "Branch previews" }
                    thead { tr { th scope="col" { "Branch" } th scope="col" { "Address" } th scope="col" { "Files" } th scope="col" { "Ends" } } }
                    tbody {
                        @for branch in &self.previews {
                            tr {
                                td { code { (Escaped(&branch.label)) } }
                                td { (Outside(&branch.url)) }
                                td { (branch.files) }
                                td { (Stamp(branch.ends)) }
                            }
                        }
                    }
                }
            }
        }
        .render(out)
    }

    fn table(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        html! {
            div.look::TABLE role="region" aria-label="Builds and deploys" tabindex=0 {
                table {
                    caption { "Builds and deploys" }
                    thead { tr { th scope="col" { "Requested" } th scope="col" { "Build" } th scope="col" { "Set off by" } th scope="col" { "State" } th scope="col" { "Deploy" } th scope="col" { "Roll back" } } }
                    tbody { @for attempt in &self.history { (Lazy(move |out: &mut dyn Sink| self.row(attempt, out))) } }
                }
            }
        }
        .render(out)
    }

    fn row(&self, attempt: &Attempt, out: &mut dyn Sink) -> Result<(), Halt> {
        let rollback = Path::act(&self.handle, "rollback");
        html! {
            tr {
                td { (Stamp(attempt.requested)) @if let Some(done) = attempt.finished { br; "ended " (Stamp(done)) } }
                td { (Log(&self.handle, attempt.build)) }
                td { (Escaped(Word(attempt.trigger))) }
                td { (Pill(attempt.state)) }
                td {
                    @if let Some(deploy) = &attempt.deploy {
                        (Id(deploy.id)) @if deploy.is_live { " " strong { "live" } }
                        br; (deploy.files) " files, " (Escaped(deploy.bytes)) ", " (Stamp(deploy.created))
                    } @else { "none" }
                }
                td {
                    @if let Some(deploy) = attempt.deploy.as_ref().filter(|deploy| !deploy.is_live) {
                        (Post { to: &rollback, name: "Roll back to this deploy", body: html! {
                            input type=(unspliced::keyword::InputType::Hidden) name="deploy" value=(Escaped(deploy.id));
                            button type=(ButtonType::Submit) { "Roll back" span.look::META { " to " (Id(deploy.id)) } }
                        } })
                    } @else { "-" }
                }
            }
        }
        .render(out)
    }
}
