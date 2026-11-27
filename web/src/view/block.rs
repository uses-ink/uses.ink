//! What more than one screen shows: the pointer a site serves from, a deploy, a build, and the
//! blocks that say there is nothing yet, or that something is wrong.

use build::{BuildId, Trigger};
use control::{Handle, JobState};
use jiff::Timestamp;
use model::{Bytes, DeployId, Diagnostic};
use unspliced::{Halt, Render, Sink, Url, html};

use crate::route::Path;
use crate::said::Said;
use crate::shell::Glyph;
use crate::style::look;
use crate::view::word::{Brief, Word};

/// The deploy a site serves, and since when.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pointer {
    /// The deploy.
    pub deploy: DeployId,
    /// When the pointer last moved.
    pub since: Timestamp,
    /// How many files it serves.
    pub files: u64,
    /// How many bytes they hold.
    pub bytes: Bytes,
}

/// What a site serves right now.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Serving {
    /// No deploy is live: nothing has been built, or nothing built has published.
    Nothing,
    /// A deploy is live and new builds publish.
    Live(Pointer),
    /// A deploy is live, and publishing is paused until `resume` (a rollback paused it).
    Paused(Pointer),
}

impl Serving {
    /// The deploy served, when one is.
    pub const fn pointer(&self) -> Option<&Pointer> {
        match self {
            Self::Nothing => None,
            Self::Live(pointer) | Self::Paused(pointer) => Some(pointer),
        }
    }
}

/// One recorded deploy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deploy {
    /// Its id.
    pub id: DeployId,
    /// When it was recorded.
    pub created: Timestamp,
    /// How many files it serves.
    pub files: u64,
    /// How many bytes they hold.
    pub bytes: Bytes,
    /// Whether the site serves it now.
    pub is_live: bool,
}

impl From<&control::Deploy> for Deploy {
    fn from(deploy: &control::Deploy) -> Self {
        Self {
            id: deploy.id,
            created: deploy.created,
            files: deploy.files,
            bytes: deploy.bytes,
            is_live: deploy.is_live,
        }
    }
}

/// One requested build and the deploy it made, if it made one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt {
    /// The build.
    pub build: BuildId,
    /// What set it off.
    pub trigger: Trigger,
    /// Where it is.
    pub state: JobState,
    /// When it was requested.
    pub requested: Timestamp,
    /// When it ended, if it has.
    pub finished: Option<Timestamp>,
    /// The deploy it made.
    pub deploy: Option<Deploy>,
}

impl Attempt {
    /// A job with the deploy it made, when the caller has it.
    pub fn of(job: &control::Job, deploy: Option<Deploy>) -> Self {
        Self {
            build: job.id,
            trigger: job.trigger,
            state: job.state,
            requested: job.requested,
            finished: job.finished,
            deploy,
        }
    }

    /// Whether the build has not ended.
    pub const fn is_running(&self) -> bool {
        matches!(self.state, JobState::Queued | JobState::Running)
    }
}

/// A time, as a machine-readable `<time>`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stamp(pub(crate) Timestamp);

impl Render for Stamp {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let text = self.0.to_string();
        html! { time.look::STAMP datetime=(&text) { (&text) } }.render(out)
    }
}

/// A build's state as a labelled pill: the word and a glyph, never colour alone.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Pill(pub(crate) JobState);

impl Render for Pill {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let glyph = match self.0 {
            JobState::Queued | JobState::Running => Glyph("...", ""),
            JobState::Succeeded => Glyph("ok", ""),
            _ => Glyph("!!", ""),
        };
        let text = Word(self.0).to_string();
        match self.0 {
            JobState::Queued | JobState::Running => {
                html! { span.look::STATE.look::RUN { (glyph) (&text) } }.render(out)
            }
            JobState::Succeeded => html! { span.look::STATE.look::LIVE { (glyph) (&text) } }.render(out),
            _ => html! { span.look::STATE.look::BAD { (glyph) (&text) } }.render(out),
        }
    }
}

/// What a screen says when it has nothing to show: what is missing, and what to do about it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Empty<'a> {
    pub(crate) title: &'a str,
    pub(crate) what: &'a str,
}

impl Render for Empty<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        html! {
            section.look::EMPTY {
                h2 { (self.title) }
                p { (self.what) }
            }
        }
        .render(out)
    }
}

/// A link to one of our own paths.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Link<'a, T>(pub(crate) &'a Path, pub(crate) T);

impl<T: Render> Render for Link<'_, T> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        html! { a href=(self.0) { (&self.1) } }.render(out)
    }
}

/// A link to a build's log.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Log<'a>(pub(crate) &'a Handle, pub(crate) BuildId);

impl Render for Log<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let path = Path::build(self.0, self.1);
        let short = self.1.to_string();
        html! { a href=(&path) { "log " code { (short.get(..8).unwrap_or(&short)) } } }.render(out)
    }
}

/// A deploy id, shortened, with its whole id for anyone who needs it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Id(pub(crate) DeployId);

impl Render for Id {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let whole = self.0.to_string();
        html! { code.look::PATH title=(&whole) { (Said(Brief(self.0))) } }.render(out)
    }
}

/// One diagnostic: severity with its glyph, code, wording, place, fix, and what caused it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Problem<'a>(pub(crate) &'a Diagnostic);

impl Render for Problem<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let chain: Vec<&Diagnostic> = self.0.chain().collect();
        html! {
            div.look::CARD {
                @for (depth, found) in chain.iter().enumerate() {
                    p.look::LINE {
                        @if depth > 0 { span.look::META { "caused by" } }
                        strong { (Severity(found.severity())) }
                        code.look::PATH { (found.code()) }
                        span { (Said(diag::Message::from(&found.problem))) }
                    }
                    @if let Some(subject) = &found.subject {
                        p.look::META { "at " code.look::PATH { (Said(subject)) } @if let Some(span) = found.span { ", " (Said(span)) } }
                    }
                    @if let Some(fix) = diag::Fix::of_problem(&found.problem) {
                        p { strong { "fix: " } (Said(fix)) }
                    }
                }
            }
        }
        .render(out)
    }
}

/// A severity with its glyph.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Severity(pub(crate) model::Severity);

impl Render for Severity {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        match self.0 {
            model::Severity::Error => Glyph("x", "error").render(out),
            model::Severity::Warning => Glyph("!", "warning").render(out),
            _ => Glyph("i", "note").render(out),
        }
    }
}

/// A form that posts one change and is answered by a redirect to the page it changed.
#[derive(Debug)]
pub(crate) struct Post<'a, B> {
    pub(crate) to: &'a Path,
    pub(crate) name: &'a str,
    pub(crate) body: B,
}

impl<B: Render> Render for Post<'_, B> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        html! {
            form method=(unspliced::keyword::Method::Post) action=(self.to) aria-label=(self.name) {
                (&self.body)
            }
        }
        .render(out)
    }
}

/// An address outside the dashboard, such as the site itself.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Outside<'a>(pub(crate) &'a str);

impl Render for Outside<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        match Url::checked(self.0) {
            Ok(url) => html! { a href=(url) { (self.0) } }.render(out),
            Err(_) => html! { code.look::PATH { (self.0) } }.render(out),
        }
    }
}
