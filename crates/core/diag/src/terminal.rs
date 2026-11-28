//! Diagnostics and notices as plain terminal text.
//!
//! ```text
//! error[link.target.ambiguous]: `[[Note]]` could be any of a/Note.md and b/Note.md
//!   at posts/x.md, bytes 12..20
//!   fix: write the path of the one you mean
//!   caused by error[...]: ...
//! ```

use std::fmt;

use model::{Diagnostic, Notice, Span, Subject};

use crate::{Fix, Message, Plain};

/// A diagnostic or a notice, laid out for a terminal without colour.
#[derive(Clone, Copy, Debug)]
pub struct Terminal<'a>(Shown<'a>);

#[derive(Clone, Copy, Debug)]
enum Shown<'a> {
    Diagnostic(&'a Diagnostic),
    Notice(&'a Notice),
}

impl<'a> From<&'a Diagnostic> for Terminal<'a> {
    fn from(diagnostic: &'a Diagnostic) -> Self {
        Self(Shown::Diagnostic(diagnostic))
    }
}

impl<'a> From<&'a Notice> for Terminal<'a> {
    fn from(notice: &'a Notice) -> Self {
        Self(Shown::Notice(notice))
    }
}

impl fmt::Display for Terminal<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Shown::Diagnostic(diagnostic) => {
                for (i, d) in diagnostic.chain().enumerate() {
                    if i > 0 {
                        out.write_str("\n  caused by ")?;
                    }
                    write!(out, "{}[{}]: {}", d.severity().name(), d.code(), Plain::text(Message::from(&d.problem)))?;
                    write!(out, "{}", Place(d.subject.as_ref(), d.span))?;
                    if let Some(fix) = Fix::of_problem(&d.problem) {
                        write!(out, "\n  fix: {fix}")?;
                    }
                }
                Ok(())
            }
            Shown::Notice(notice) => {
                write!(out, "notice[{}]: {}", notice.code(), Plain::text(Message::from(&notice.announcement)))?;
                write!(out, "{}", Place(notice.subject.as_ref(), notice.span))?;
                match Fix::of_announcement(&notice.announcement) {
                    Some(fix) => write!(out, "\n  fix: {fix}"),
                    None => Ok(()),
                }
            }
        }
    }
}

/// `  at <subject>, <span>`, as much of it as is known.
struct Place<'a>(Option<&'a Subject>, Option<Span>);

impl fmt::Display for Place<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.0, self.1) {
            (Some(subject), Some(span)) => write!(out, "\n  at {}, {span}", Plain::flat(subject)),
            (Some(subject), None) => write!(out, "\n  at {}", Plain::flat(subject)),
            (None, Some(span)) => write!(out, "\n  at {span}"),
            (None, None) => Ok(()),
        }
    }
}

#[cfg(test)]
mod test {
    use model::{ContentPath, Diagnostic, Notice, Problem, Span, Subject};

    use super::Terminal;

    fn path(text: &str) -> ContentPath {
        ContentPath::parse(text).unwrap()
    }

    #[test]
    fn lays_out_a_diagnostic_and_its_cause() {
        let cause = Diagnostic::new(Problem::SourceFileUnreadable { path: path("b.md"), reason: "denied".to_owned() });
        let ambiguous = Problem::LinkTargetAmbiguous {
            written: "[[Note]]".to_owned(),
            candidates: vec![path("a/Note.md"), path("b/Note.md")],
        };
        let diagnostic = Diagnostic::new(ambiguous)
            .on(Subject::File(path("posts/x.md")))
            .at(Span::new(12, 20).unwrap())
            .caused_by(cause);
        assert_eq!(
            Terminal::from(&diagnostic).to_string(),
            "error[link.target.ambiguous]: `[[Note]]` could be any of a/Note.md and b/Note.md\n  \
             at posts/x.md, bytes 12..20\n  \
             fix: write the path of the one you mean\n  \
             caused by error[source.file.unreadable]: b.md could not be read: denied"
        );
    }

    #[test]
    fn a_name_cannot_write_escape_sequences_to_the_terminal() {
        let hostile = path("\u{1b}]52;c;AAAA\u{7}.md");
        let diagnostic =
            Diagnostic::new(Problem::SourceFileUnreadable { path: hostile.clone(), reason: "denied".into() })
                .on(Subject::File(hostile));
        let shown = Terminal::from(&diagnostic).to_string();
        assert!(!shown.contains('\u{1b}') && !shown.contains('\u{7}'), "{shown:?}");
        assert!(shown.contains("\\u{1b}]52;c;AAAA\\u{7}.md could not be read"), "{shown}");
        assert!(shown.contains("at \\u{1b}]52;c;AAAA\\u{7}.md"), "{shown}");
    }

    #[test]
    fn lays_out_a_notice() {
        let notice = Notice::new(model::Announcement::SiteHomeGenerated).on(Subject::Site);
        assert_eq!(
            Terminal::from(&notice).to_string(),
            "notice[site.home.generated]: no home page was found, so the home page lists every page\n  \
             at the site\n  \
             fix: add index.md or README.md at the root, or pin `site.home`"
        );
    }
}
