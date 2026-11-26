//! What interpreting has said so far: decisions, indexed by code and subject so later inferrers
//! and the assembly read them back, and the diagnostics and notices that came with them.

use std::collections::BTreeMap;
use std::fmt;

use model::{
    Announcement, Decided, Decision, DecisionCode, DefaultReason, Diagnostic, Evidence, Layout, Notice, PageKind, Root,
    Severity, Subject,
};

use crate::error::Broken;
use crate::interpret::ladder::Code;
use crate::pins::Pins;

#[derive(Debug, Default)]
pub(crate) struct Record {
    pub(crate) decisions: Vec<Decision>,
    index: BTreeMap<(Subject, DecisionCode), usize>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) notices: Vec<Notice>,
}

impl Record {
    /// Records `decision`, and the notice its default calls for. A second decision of one code
    /// on one subject is a bug in the engine.
    pub(crate) fn decide(&mut self, decision: Decision) {
        let key = (decision.subject.clone(), decision.code());
        if self.index.contains_key(&key) {
            let (subject, decision) = key;
            self.broken(Broken::Repeated { decision, subject });
            return;
        }
        if let Some(announcement) = announcement(&decision) {
            self.notices.push(Notice::new(announcement).on(decision.subject.clone()));
        }
        self.index.insert(key, self.decisions.len());
        self.decisions.push(decision);
    }

    /// The value decided for `C` on `subject`.
    pub(crate) fn get<C: Code>(&self, subject: &Subject) -> Option<&C::Value> {
        self.decision::<C>(subject).and_then(|decision| C::unwrap(&decision.decided))
    }

    pub(crate) fn decision<C: Code>(&self, subject: &Subject) -> Option<&Decision> {
        let at = self.index.get(&(subject.clone(), C::CODE))?;
        self.decisions.get(*at)
    }

    /// The pinned value of `C` on `subject`, as a ladder's first rung. A pin whose value is of
    /// another code is a bug in whoever implements [`Pins`].
    pub(crate) fn pinned<C: Code>(&mut self, pins: &impl Pins, subject: &Subject) -> Option<(C::Value, Evidence)> {
        let pin = pins.pin(C::CODE, subject)?;
        if let Some(value) = C::unwrap(&pin.decided) {
            return Some((value.clone(), Evidence::Config { span: pin.span }));
        }
        let subject = subject.clone();
        self.broken(Broken::Mismatched { decision: C::CODE, found: pin.decided.code(), subject });
        None
    }

    pub(crate) fn diagnose(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    pub(crate) fn notice(&mut self, notice: Notice) {
        self.notices.push(notice);
    }

    /// A rule of the engine's own did not hold.
    pub(crate) fn broken(&mut self, broken: Broken) {
        self.diagnose(broken.into());
    }

    /// The value decided for `C` on `subject`, or the bug of it missing, recorded.
    pub(crate) fn needed<C: Code>(&mut self, subject: &Subject) -> Option<C::Value> {
        let value = self.get::<C>(subject).cloned();
        if value.is_none() {
            self.broken(Broken::Missing { decision: C::CODE, subject: subject.clone() });
        }
        value
    }

    /// Whether an error stops the build: all or nothing (open.md Q6).
    pub(crate) fn stops(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity() == Severity::Error)
    }
}

/// The notice a default calls for. Defaults that only mean "nothing to do" (nothing ignored, no
/// tags, never updated, a public page published) are recorded as decisions and announce nothing,
/// or every file would; an undated post is announced by `page.date` itself.
fn announcement(decision: &Decision) -> Option<Announcement> {
    let Evidence::Default { reason } = decision.evidence else { return None };
    let chosen =
        || Announcement::NoticeDefaultChosen { decision: decision.code(), value: Shown(&decision.decided).to_string() };
    match (&decision.decided, reason) {
        (Decided::Home(_), _) => Some(Announcement::SiteHomeGenerated),
        (
            Decided::Root(_)
            | Decided::Layout(_)
            | Decided::Kind(_)
            | Decided::Slug(_)
            | Decided::Nav(_)
            | Decided::SiteTitle(_)
            | Decided::Authors(_)
            | Decided::Language(_),
            _,
        )
        | (Decided::Publish(_), DefaultReason::NotSelected) => Some(chosen()),
        _ => None,
    }
}

/// A decided value as a person reads it.
#[derive(Debug)]
pub(crate) struct Shown<'a>(pub(crate) &'a Decided);

impl fmt::Display for Shown<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let yes_no = |yes: bool, a: &str, b: &str| if yes { a.to_owned() } else { b.to_owned() };
        let listed = |items: Vec<String>, none: &str| if items.is_empty() { none.to_owned() } else { items.join(", ") };
        let text = match self.0 {
            Decided::Root(Root::Top) => "the top of the snapshot".to_owned(),
            Decided::Root(Root::Folder(folder)) => format!("`{folder}/`"),
            Decided::Layout(layout) => Kinds::Layout(*layout).to_string(),
            Decided::Kind(kind) => Kinds::Page(*kind).to_string(),
            Decided::Ignored(ignored) => yes_no(*ignored, "ignored", "kept"),
            Decided::Publish(publish) => yes_no(*publish, "published", "not published"),
            Decided::Home(Some(path)) => path.to_string(),
            Decided::Home(None) => "a generated listing".to_owned(),
            Decided::Title(title) | Decided::SiteTitle(title) => format!("“{title}”"),
            Decided::Date(when) => when.map_or_else(|| "undated".to_owned(), |when| when.to_string()),
            Decided::Updated(when) => when.map_or_else(|| "never updated".to_owned(), |when| when.to_string()),
            Decided::Slug(slug) => format!("`{slug}`"),
            Decided::Tags(tags) => listed(tags.iter().map(|t| t.written.clone()).collect(), "no tags"),
            Decided::Nav(paths) => listed(paths.iter().map(ToString::to_string).collect(), "empty"),
            Decided::Summary(text) | Decided::Description(text) => {
                text.as_ref().map_or_else(|| "none".to_owned(), |text| format!("“{text}”"))
            }
            Decided::Authors(authors) => listed(authors.iter().map(ToString::to_string).collect(), "no one"),
            Decided::Language(language) => format!("`{language}`"),
            other => format!("{other:?}"),
        };
        out.write_str(&text)
    }
}

/// Layouts and page kinds as words.
#[derive(Clone, Copy, Debug)]
enum Kinds {
    Layout(Layout),
    Page(PageKind),
}

impl fmt::Display for Kinds {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout(Layout::Blog) => out.write_str("a blog"),
            Self::Layout(Layout::Vault) => out.write_str("a vault"),
            Self::Layout(Layout::Docs) => out.write_str("docs"),
            Self::Layout(Layout::Flat) => out.write_str("flat"),
            Self::Page(PageKind::Home) => out.write_str("the home page"),
            Self::Page(PageKind::Post) => out.write_str("a post"),
            Self::Page(PageKind::Page) => out.write_str("a page"),
            Self::Layout(other) => write!(out, "{other:?}"),
            Self::Page(other) => write!(out, "{other:?}"),
        }
    }
}
