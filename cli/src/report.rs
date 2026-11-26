//! Diagnostics and notices as a person reads them (conventions.md, CLI output), and as NDJSON
//! events with `--json`. The one place `annotate-snippets` is used: a diagnostic with a span shows
//! its source line under it.

use std::fmt;

use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet};
use diag::{Fix, Message, Terminal};
use engine::Blobs;
use host_native::Memory;
use model::{ContentPath, Diagnostic, Notice, Severity, Snapshot, Span, Subject};
use serde::Serialize;

/// The files of the folder, to show the lines a span points at.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Texts<'a> {
    pub(crate) snapshot: Option<&'a Snapshot>,
    pub(crate) store: Option<&'a Memory>,
}

impl Texts<'_> {
    /// No files: diagnostics from before the folder was read.
    pub(crate) const NONE: Texts<'static> = Texts { snapshot: None, store: None };

    /// The text of `path`, when it is UTF-8.
    fn text(&self, path: &ContentPath) -> Option<String> {
        let entry = self.snapshot?.entry(path)?;
        let bytes = self.store?.read(&entry.blob).ok()?;
        String::from_utf8(bytes.into_owned()).ok()
    }

    /// The 1-based line `span` starts on in `path`.
    fn line(&self, path: &ContentPath, span: Span) -> Option<usize> {
        let text = self.text(path)?;
        let before = text.get(..span.start())?;
        Some(before.matches('\n').count() + 1)
    }
}

/// One diagnostic laid out for a terminal.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Shown<'a> {
    pub(crate) diagnostic: &'a Diagnostic,
    pub(crate) texts: Texts<'a>,
}

impl fmt::Display for Shown<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let diagnostic = self.diagnostic;
        let (Some(Subject::File(path)), Some(span)) = (&diagnostic.subject, diagnostic.span) else {
            return write!(out, "{}", Terminal::from(diagnostic));
        };
        let Some(text) = self.texts.text(path).filter(|text| span.end() <= text.len()) else {
            return write!(out, "{}", Terminal::from(diagnostic));
        };
        let level = match diagnostic.severity() {
            Severity::Error => Level::ERROR,
            Severity::Warning => Level::WARNING,
            _ => Level::NOTE,
        };
        let message = Message::from(diagnostic).to_string();
        let report = [level.primary_title(message).id(diagnostic.code()).element(
            Snippet::source(text.as_str())
                .path(path.as_str())
                .annotation(AnnotationKind::Primary.span(span.start()..span.end())),
        )];
        write!(out, "{}", Renderer::plain().render(&report))?;
        if let Some(fix) = Fix::of_problem(&diagnostic.problem) {
            write!(out, "\n  fix: {fix}")?;
        }
        if let Some(cause) = &diagnostic.cause {
            write!(out, "\n  caused by {}", Terminal::from(cause.as_ref()))?;
        }
        Ok(())
    }
}

/// A notice on one line: `note[code]: wording (where)`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Noted<'a>(pub(crate) &'a Notice);

impl fmt::Display for Noted<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "note[{}]: {}", self.0.code(), Message::from(self.0))?;
        match &self.0.subject {
            Some(subject) => write!(out, " ({subject})"),
            None => Ok(()),
        }
    }
}

/// A diagnostic or a notice as an NDJSON event (research/flow.md section 7).
#[derive(Debug, Serialize)]
pub(crate) struct Event {
    schema: &'static str,
    code: &'static str,
    severity: &'static str,
    file: Option<String>,
    line: Option<usize>,
    what: String,
    why: Option<String>,
    fix: Option<String>,
}

impl Event {
    const SCHEMA: &'static str = "uses.event/v1";

    pub(crate) fn diagnostic(diagnostic: &Diagnostic, texts: Texts<'_>) -> Self {
        let file = match &diagnostic.subject {
            Some(Subject::File(path)) => Some(path),
            _ => None,
        };
        Self {
            schema: Self::SCHEMA,
            code: diagnostic.code(),
            severity: diagnostic.severity().name(),
            file: file.map(ToString::to_string),
            line: file.zip(diagnostic.span).and_then(|(path, span)| texts.line(path, span)),
            what: Message::from(diagnostic).to_string(),
            why: diagnostic.cause.as_deref().map(|cause| Message::from(cause).to_string()),
            fix: Fix::of_problem(&diagnostic.problem).map(|fix| fix.to_string()),
        }
    }

    pub(crate) fn notice(notice: &Notice) -> Self {
        Self {
            schema: Self::SCHEMA,
            code: notice.code(),
            severity: "notice",
            file: match &notice.subject {
                Some(Subject::File(path)) => Some(path.to_string()),
                _ => None,
            },
            line: None,
            what: Message::from(notice).to_string(),
            why: None,
            fix: Fix::of_announcement(&notice.announcement).map(|fix| fix.to_string()),
        }
    }
}
