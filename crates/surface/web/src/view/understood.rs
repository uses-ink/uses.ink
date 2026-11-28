//! "What we understood": the decisions behind the site, each with the evidence that chose it,
//! in the order `ink explain` prints (flow.md, section 3). A row's "why" is a `<details>`; the
//! whole ladder replayed is the next page ([`Why`](crate::view::Why)).

use std::fmt;

use build::Source;
use control::Handle;
use model::decision::Basis;
use model::{AnyCode, AnyDecision, DeployId, Observed};
use unspliced::{Halt, Lazy, Render, Sink, html};

use crate::escaped::Escaped;
use crate::form::Asked;
use crate::route::{Only, Path, Screen};
use crate::shell::{Frame, Glyph, View};
use crate::style::look;
use crate::view::block::{Empty, Id, Link};
use crate::view::word::Word;

/// The kinds of decision, in the order the tables come: what turns out wrong most often first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Group {
    /// Decisions about one page: its kind, title, date, tags.
    Pages,
    /// Decisions about one file: whether it is published at all.
    Files,
    /// Decisions about the site as a whole.
    Site,
    /// A decision whose code starts with none of these.
    Other,
}

impl Group {
    /// Every group, in table order.
    pub const ALL: [Self; 4] = [Self::Pages, Self::Files, Self::Site, Self::Other];

    /// The group a decision code belongs to, by the noun it starts with.
    pub fn of(code: AnyCode) -> Self {
        match code.name().split('.').next() {
            Some("page") => Self::Pages,
            Some("file") => Self::Files,
            Some("site") => Self::Site,
            _ => Self::Other,
        }
    }
}

impl fmt::Display for Group {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Pages => "Pages and posts",
            Self::Files => "Files",
            Self::Site => "The site",
            Self::Other => "Other decisions",
        })
    }
}

/// What the "What we understood" screen shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Understood {
    /// Where the page sits.
    pub frame: Frame,
    /// The site's handle.
    pub handle: Handle,
    /// The deploy these decisions belong to; none before the first build.
    pub deploy: Option<DeployId>,
    /// Where the content comes from.
    pub source: Source,
    /// What the source could and could not do.
    pub observed: Observed,
    /// Every decision of the deploy, in the order the engine decided them.
    pub decisions: Vec<AnyDecision>,
    /// Whether the page lists every decision or only those to check.
    pub only: Only,
}

impl Understood {
    /// A screen of `decisions` for `handle`.
    pub fn new(handle: Handle, source: Source, decisions: Vec<AnyDecision>) -> Self {
        Self {
            frame: Frame::of(&handle, Screen::Understood),
            handle,
            deploy: None,
            source,
            observed: Observed::default(),
            decisions,
            only: Only::Everything,
        }
    }

    /// Whether a decision is one to check: nothing said its value, so the default was taken.
    pub fn is_check(decision: &AnyDecision) -> bool {
        decision.evidence().basis() == Basis::Assumed
    }

    fn checks(&self) -> usize {
        self.decisions.iter().filter(|decision| Self::is_check(decision)).count()
    }

    fn shown(&self, group: Group) -> Vec<&AnyDecision> {
        self.decisions
            .iter()
            .filter(|decision| Group::of(decision.code()) == group)
            .filter(|decision| self.only == Only::Everything || Self::is_check(decision))
            .collect()
    }
}

impl View for Understood {
    fn frame(&self) -> &Frame {
        &self.frame
    }

    fn title(&self) -> impl fmt::Display + '_ {
        "What we understood"
    }

    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let (all, check) = (Path::of(Screen::Understood, &self.handle), Path::check(&self.handle));
        html! {
            p.look::LEDE { "Everything uses.ink inferred about " (Escaped(&self.handle)) ", and why. To change a value, pin it in `ink.toml` in your own source; this page writes nothing." }
            p.look::LINE {
                "reading " code.look::PATH { (Escaped(Describe(&self.source))) }
                @if let Some(deploy) = self.deploy { " from deploy " (Id(deploy)) }
            }
            @if self.decisions.is_empty() {
                (Empty {
                    title: "No decisions yet",
                    what: "Decisions are recorded by a build. Build the site from the Site screen, then come back to see what was understood and why.",
                })
            } @else {
                p.look::LINE {
                    (self.decisions.len()) " decisions, "
                    @if self.checks() > 0 { span.look::FLAG { (self.checks()) " worth a look" } } @else { "none worth a look" }
                    @if self.only == Only::Check { (Link(&all, "Show every decision")) } @else { (Link(&check, "Show only decisions to check")) }
                }
                @if self.only == Only::Everything { (Lazy(|out: &mut dyn Sink| self.looks(out))) }
                @for group in Group::ALL { (Lazy(move |out: &mut dyn Sink| self.table(group, out))) }
            }
        }
        .render(out)
    }
}

impl Understood {
    /// The decisions to check, first and in the red pen, each with the line that pins it.
    fn looks(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let checks: Vec<&AnyDecision> = self.decisions.iter().filter(|decision| Self::is_check(decision)).collect();
        if checks.is_empty() {
            return Ok(());
        }
        html! {
            section aria-labelledby="look" {
                h2 #look { "Worth a look" }
                @for decision in &checks { (Look { handle: &self.handle, decision }) }
            }
        }
        .render(out)
    }
}

/// One decision to check, as an item: what it is about, why it is a guess, and how to pin it.
#[derive(Clone, Copy, Debug)]
struct Look<'a> {
    handle: &'a Handle,
    decision: &'a AnyDecision,
}

impl Render for Look<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let decision = self.decision;
        let (code, subject) = (decision.code(), decision.subject());
        let replay = Path::why(self.handle, &Asked { code, subject: subject.clone() });
        let because = diag::Because(decision.evidence());
        let pin = diag::PinCommand(&code, &subject);
        let (about, name) = (Escaped(&subject), code.name());
        html! {
            div.look::ITEM {
                span.look::RING aria-hidden="true" { "!" }
                div {
                    p { code.look::PATH { (about) } " " code.look::PATH { (name) } ": " (Escaped(diag::Value(decision))) }
                    p.look::WHY { "chosen " (Escaped(because)) }
                    p.look::META { "to pin it: " code.look::PATH { (Escaped(pin)) } " " (Link(&replay, "Replay the ladder")) }
                }
            }
        }
        .render(out)
    }
}

impl Understood {
    fn table(&self, group: Group, out: &mut dyn Sink) -> Result<(), Halt> {
        let rows = self.shown(group);
        if rows.is_empty() {
            return Ok(());
        }
        let caption = group.to_string();
        html! {
            div.look::TABLE role="region" aria-label=(&caption) tabindex=0 {
                table {
                    caption { (&caption) }
                    thead { tr { th scope="col" { "About" } th scope="col" { "Decision" } th scope="col" { "Value" } th scope="col" { "From" } th scope="col" { "Check" } th scope="col" { "Why" } } }
                    tbody {
                        @for decision in &rows { (Row { handle: &self.handle, decision }) }
                    }
                }
            }
        }
        .render(out)
    }
}

/// A source in a line.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Describe<'a>(pub(crate) &'a Source);

impl fmt::Display for Describe<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Source::Folder(path) => write!(out, "folder {}", path.display()),
            Source::Git { url, revision: Some(revision) } => write!(out, "{url} at {}", revision.as_str()),
            Source::Git { url, revision: None } => write!(out, "{url}, the default branch"),
            _ => out.write_str("a source this version cannot show"),
        }
    }
}

/// One decision as a table row.
#[derive(Clone, Copy, Debug)]
struct Row<'a> {
    handle: &'a Handle,
    decision: &'a AnyDecision,
}

impl Render for Row<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let decision = self.decision;
        let check = Understood::is_check(decision);
        let (code, subject) = (decision.code(), decision.subject());
        let asked = Asked { code, subject: subject.clone() };
        let replay = Path::why(self.handle, &asked);
        let because = diag::Because(decision.evidence());
        let pin = diag::PinCommand(&code, &subject);
        let about = Escaped(&subject);
        let say = html! {
            td { code.look::PATH { (about) } }
            td { code.look::PATH { (code.name()) } }
            td { (Escaped(diag::Value(decision))) }
            td { (Escaped(because)) }
            td { @if check { strong { (Glyph("!", "check")) } } @else { "ok" } }
            td {
                details {
                    summary { "Show why" }
                    p { "Chosen " (Escaped(because)) "." }
                    p { "To pin it: " code.look::PATH { (Escaped(pin)) } }
                    p { (Link(&replay, "Replay the ladder")) }
                }
            }
        };
        if check { html! { tr.look::CHECK { (say) } }.render(out) } else { html! { tr { (say) } }.render(out) }
    }
}

impl fmt::Display for Word<Basis> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self.0 {
            Basis::Stated => "you said it",
            Basis::Inferred => "read from your content",
            Basis::Assumed => "assumed",
            _ => "a basis this version cannot show",
        })
    }
}
