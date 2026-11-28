//! Problems and notices. Their codes, fields and wording come from `crates/core/diag/catalog.toml`;
//! [`Problem`] and [`Announcement`] are generated from it, the words live in `uses-ink-diag`.

use serde::Serialize;

use crate::{Announcement, Problem, Span, Subject};

/// How bad a problem is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Severity {
    /// The build cannot go on, or would publish something wrong.
    Error,
    /// The build goes on; the author should look.
    Warning,
    /// Worth knowing; nothing is wrong.
    Note,
}

impl Severity {
    /// As the terminal and NDJSON write it: `error`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Note => "note",
        }
    }
}

/// Whose move it is to fix a problem.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Fault {
    /// The author: their content or their `ink.toml`.
    Author,
    /// The source: a forge, a disk, a network.
    Source,
    /// Whoever runs the instance: limits, credentials, configuration.
    Operator,
    /// uses.ink itself: a bug.
    Ours,
}

impl Fault {
    /// As NDJSON writes it: `author`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Author => "author",
            Self::Source => "source",
            Self::Operator => "operator",
            Self::Ours => "ours",
        }
    }
}

/// A problem, where it is, and what caused it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Diagnostic {
    /// What is wrong, with its code and typed fields.
    pub problem: Problem,
    /// What it is about: the site, a file, or nothing in particular.
    pub subject: Option<Subject>,
    /// Where in the subject's file, when a place is known.
    pub span: Option<Span>,
    /// The problem that caused it, shown as its `caused by` chain.
    pub cause: Option<Box<Diagnostic>>,
}

impl Diagnostic {
    /// `problem`, about nothing in particular, with no place and no cause.
    pub fn new(problem: Problem) -> Self {
        Self { problem, subject: None, span: None, cause: None }
    }

    /// The same problem, about `subject`.
    #[must_use]
    pub fn on(mut self, subject: Subject) -> Self {
        self.subject = Some(subject);
        self
    }

    /// The same problem, at `span` of its subject's file.
    #[must_use]
    pub fn at(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// The same problem, caused by `cause`.
    #[must_use]
    pub fn caused_by(mut self, cause: Self) -> Self {
        self.cause = Some(Box::new(cause));
        self
    }

    /// Its problem's code, `noun.noun.reason`.
    pub const fn code(&self) -> &'static str {
        self.problem.code()
    }

    /// How bad its problem is.
    pub const fn severity(&self) -> Severity {
        self.problem.severity()
    }

    /// Whose move it is to fix it.
    pub const fn fault(&self) -> Fault {
        self.problem.fault()
    }

    /// Itself, then its cause, then the cause's cause.
    pub fn chain(&self) -> impl Iterator<Item = &Self> {
        std::iter::successors(Some(self), |d| d.cause.as_deref())
    }
}

impl From<Problem> for Diagnostic {
    fn from(problem: Problem) -> Self {
        Self::new(problem)
    }
}

/// Something worth saying that is not a problem, such as a default being chosen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Notice {
    /// What it says, with its code and typed fields.
    pub announcement: Announcement,
    /// What it is about: the site, a file, or nothing in particular.
    pub subject: Option<Subject>,
    /// Where in the subject's file, when a place is known.
    pub span: Option<Span>,
}

impl Notice {
    /// `announcement`, about nothing in particular, with no place.
    pub fn new(announcement: Announcement) -> Self {
        Self { announcement, subject: None, span: None }
    }

    /// The same notice, about `subject`.
    #[must_use]
    pub fn on(mut self, subject: Subject) -> Self {
        self.subject = Some(subject);
        self
    }

    /// The same notice, at `span` of its subject's file.
    #[must_use]
    pub fn at(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    /// Its announcement's code, `noun.noun.reason`.
    pub const fn code(&self) -> &'static str {
        self.announcement.code()
    }
}

impl From<Announcement> for Notice {
    fn from(announcement: Announcement) -> Self {
        Self::new(announcement)
    }
}

/// Where a plugin says what it found while it worked: the native stand-in for the
/// `ink:host/diagnostic` capability. The engine's render records into its output, a component
/// records through its host, a test records into a [`Said`].
pub trait Sink {
    /// Records a problem.
    fn diagnose(&mut self, diagnostic: Diagnostic);
    /// Records an announcement.
    fn notice(&mut self, notice: Notice);
}

/// Everything said, in order: a [`Sink`] that keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Said {
    /// Problems, such as `math.syntax.invalid`.
    pub diagnostics: Vec<Diagnostic>,
    /// Notices, such as `code.language.unknown`.
    pub notices: Vec<Notice>,
}

impl Sink for Said {
    fn diagnose(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    fn notice(&mut self, notice: Notice) {
        self.notices.push(notice);
    }
}
