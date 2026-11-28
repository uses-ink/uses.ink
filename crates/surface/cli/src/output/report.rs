//! Diagnostics and notices as blocks (decision 0061): each is a typed value rendered as text once,
//! through the theme, and serialized as an NDJSON event with `--json` (research/flow.md section
//! 7). The one place `annotate-snippets` is used: a diagnostic with a span shows its source line
//! under it, styled by the same theme.

use annotate_snippets::{AnnotationKind, Level, Renderer, Snippet};
use diag::{Fix, Message};
use engine::Blobs;
use host_native::Memory;
use model::{ContentPath, Diagnostic, Notice, Severity, Snapshot, Span, Subject};
use serde::Serialize;

use crate::output::term::{Doc, Line, Look, Piece, Render, Role, Safe, Strength, Tree, Width};

/// The files of the folder, to show the lines a span points at.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Texts<'a> {
    pub(crate) snapshot: Option<&'a Snapshot>,
    pub(crate) store: Option<&'a Memory>,
}

impl Texts<'_> {
    /// No files: diagnostics from before the folder was read.
    pub(crate) const NONE: Texts<'static> = Texts { snapshot: None, store: None };

    /// The text of `path`, when it is UTF-8, with its control characters made safe to print in a
    /// way that keeps every byte offset.
    fn text(&self, path: &ContentPath) -> Option<String> {
        let entry = self.snapshot?.entry(path)?;
        let bytes = self.store?.read(&entry.blob).ok()?;
        String::from_utf8(bytes.into_owned()).ok().map(|text| Safe(&text).offsets())
    }

    /// The 1-based line `span` starts on in `path`.
    fn line(&self, path: &ContentPath, span: Span) -> Option<usize> {
        let text = self.text(path)?;
        let before = text.get(..span.start())?;
        Some(before.matches('\n').count() + 1)
    }
}

/// The first line of a diagnostic or a notice: `error[code]: wording`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Label<'a> {
    severity: Severity,
    code: &'a str,
    message: &'a str,
}

impl<'a> Label<'a> {
    /// The label of an error that stopped a command.
    pub(crate) const fn error(code: &'a str, message: &'a str) -> Self {
        Self { severity: Severity::Error, code, message }
    }

    /// The role a severity is written in.
    const fn role(severity: Severity) -> Role {
        match severity {
            Severity::Error => Role::Bad,
            Severity::Warning => Role::Warn,
            _ => Role::Quiet,
        }
    }

    pub(crate) fn line(self) -> Line {
        Line::new()
            .strong(Self::role(self.severity), format!("{}[{}]", self.severity.name(), self.code))
            .with(Role::Plain, ": ")
            .then(Line::marked(Role::Heading, self.message))
    }
}

/// `fix: text`, indented.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Advice<'a>(pub(crate) &'a str);

impl Advice<'_> {
    pub(crate) fn line(self) -> Line {
        Line::new().pad(2).with(Role::Literal, "fix:").pad(1).then(Line::marked(Role::Plain, self.0))
    }
}

/// One diagnostic laid out for a terminal.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Shown<'a> {
    pub(crate) diagnostic: &'a Diagnostic,
    pub(crate) texts: Texts<'a>,
}

impl Shown<'_> {
    /// The source line under the diagnostic, laid out by `annotate-snippets` in the theme's styles,
    /// when the diagnostic has a span in a file whose text is known.
    fn snippet(&self, look: Look) -> Option<Vec<Line>> {
        let diagnostic = self.diagnostic;
        let (Some(Subject::File(path)), Some(span)) = (&diagnostic.subject, diagnostic.span) else { return None };
        let text = self.texts.text(path).filter(|text| span.end() <= text.len())?;
        let (level, role) = match diagnostic.severity() {
            Severity::Error => (Level::ERROR, Role::Bad),
            Severity::Warning => (Level::WARNING, Role::Warn),
            _ => (Level::NOTE, Role::Quiet),
        };
        let theme = look.theme();
        let (plain, strong) = (Strength::Normal, Strength::Strong);
        let mut renderer = Renderer::plain()
            .error(theme.style(Role::Bad, strong))
            .warning(theme.style(Role::Warn, strong))
            .note(theme.style(role, strong))
            .line_num(theme.style(Role::Place, plain))
            .context(theme.style(Role::Place, plain))
            .emphasis(theme.style(Role::Heading, plain));
        if let Width::Columns(columns) = look.width {
            renderer = renderer.term_width(columns);
        }
        let message = Message::from(diagnostic).to_string();
        let report = [level.primary_title(message).id(diagnostic.code()).element(
            Snippet::source(text.as_str())
                .path(path.as_str())
                .annotation(AnnotationKind::Primary.span(span.start()..span.end())),
        )];
        Some(
            renderer
                .render(&report)
                .lines()
                .map(|rendered| {
                    let mut line = Line::new();
                    line.push(Piece::ansi(rendered.to_owned()));
                    line
                })
                .collect(),
        )
    }

    /// Where the diagnostic is, when no snippet shows it: `  --> subject, span`.
    fn place(&self) -> Option<Line> {
        let diagnostic = self.diagnostic;
        let subject = match &diagnostic.subject {
            None | Some(Subject::Site) => return None,
            Some(subject) => subject.to_string(),
        };
        let at = diagnostic.span.map_or_else(String::new, |span| format!(", {span}"));
        Some(Line::new().pad(2).with(Role::Place, format!("--> {subject}{at}")))
    }
}

impl Render for Shown<'_> {
    fn render(&self, into: &mut Doc) {
        let diagnostic = self.diagnostic;
        if let Some(lines) = self.snippet(into.look()) {
            for line in lines {
                into.push(line);
            }
        } else {
            let message = Message::from(diagnostic).to_string();
            into.push(Label { severity: diagnostic.severity(), code: diagnostic.code(), message: &message }.line());
            if let Some(place) = self.place() {
                into.push(place);
            }
        }
        if let Some(fix) = Fix::of_problem(&diagnostic.problem) {
            into.push(Advice(&fix.to_string()).line());
        }
        let mut causes = Tree::new(Line::new().with(Role::Plain, "caused by")).indent(2);
        for cause in diagnostic.chain().skip(1) {
            let message = Message::from(&cause.problem).to_string();
            let label = Label { severity: cause.severity(), code: cause.code(), message: &message };
            causes = causes.row(vec![label.line()]);
        }
        if !causes.is_empty() {
            causes.render(into);
        }
        into.blank();
    }
}

/// A notice: `note[code]: wording`, then its fix when it has one.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Noted<'a>(pub(crate) &'a Notice);

impl Render for Noted<'_> {
    fn render(&self, into: &mut Doc) {
        let notice = self.0;
        let message = Message::from(notice).to_string();
        into.push(Label { severity: Severity::Note, code: notice.code(), message: &message }.line());
        if let Some(fix) = Fix::of_announcement(&notice.announcement) {
            into.push(Advice(&fix.to_string()).line());
        }
        into.blank();
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
