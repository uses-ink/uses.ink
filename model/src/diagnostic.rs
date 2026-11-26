//! Problems and notices. Their codes, fields and wording come from `diag/catalog.toml`;
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
    pub problem: Problem,
    pub subject: Option<Subject>,
    pub span: Option<Span>,
    pub cause: Option<Box<Diagnostic>>,
}

impl Diagnostic {
    pub fn new(problem: Problem) -> Self {
        Self { problem, subject: None, span: None, cause: None }
    }

    #[must_use]
    pub fn on(mut self, subject: Subject) -> Self {
        self.subject = Some(subject);
        self
    }

    #[must_use]
    pub fn at(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    #[must_use]
    pub fn caused_by(mut self, cause: Self) -> Self {
        self.cause = Some(Box::new(cause));
        self
    }

    pub const fn code(&self) -> &'static str {
        self.problem.code()
    }

    pub const fn severity(&self) -> Severity {
        self.problem.severity()
    }

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
    pub announcement: Announcement,
    pub subject: Option<Subject>,
    pub span: Option<Span>,
}

impl Notice {
    pub fn new(announcement: Announcement) -> Self {
        Self { announcement, subject: None, span: None }
    }

    #[must_use]
    pub fn on(mut self, subject: Subject) -> Self {
        self.subject = Some(subject);
        self
    }

    #[must_use]
    pub fn at(mut self, span: Span) -> Self {
        self.span = Some(span);
        self
    }

    pub const fn code(&self) -> &'static str {
        self.announcement.code()
    }
}

impl From<Announcement> for Notice {
    fn from(announcement: Announcement) -> Self {
        Self::new(announcement)
    }
}
