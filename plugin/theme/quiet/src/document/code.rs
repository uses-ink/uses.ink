//! Code blocks, highlighted by the host's [`Highlight`] capability (decision 0039): each run's
//! [`Capture`] becomes a class of the theme's code stylesheet, so markup holds no style and no
//! text the theme did not escape.

use std::error::Error;
use std::fmt;

use engine::Sink;
use model::highlight::{Capture, Highlight, Unhighlighted};
use model::{Announcement, Diagnostic, Notice, Problem};
use unspliced::{Class, html};

use super::Writer;
use crate::style::code;

/// The class the theme styles a capture with.
#[derive(Clone, Copy, Debug)]
struct Shade(Capture);

impl Shade {
    /// `None` for a capture this theme does not style yet: `Capture` is `non_exhaustive`, so the
    /// match has a fallback arm, and the test below holds every capture in `Capture::ALL` to a class.
    fn class(self) -> Option<Class> {
        Some(match self.0 {
            Capture::Attribute => code::ATTRIBUTE,
            Capture::Comment => code::COMMENT,
            Capture::Constant => code::CONSTANT,
            Capture::Constructor => code::CONSTRUCTOR,
            Capture::Deleted => code::DELETED,
            Capture::Embedded => code::EMBEDDED,
            Capture::Emphasis => code::EMPHASIS,
            Capture::Error => code::ERROR,
            Capture::Function => code::FUNCTION,
            Capture::Heading => code::HEADING,
            Capture::Inserted => code::INSERTED,
            Capture::Keyword => code::KEYWORD,
            Capture::Label => code::LABEL,
            Capture::Link => code::LINK,
            Capture::Literal => code::LITERAL,
            Capture::Macro => code::MACRO,
            Capture::Module => code::MODULE,
            Capture::Number => code::NUMBER,
            Capture::Operator => code::OPERATOR,
            Capture::Property => code::PROPERTY,
            Capture::Punctuation => code::PUNCTUATION,
            Capture::Strike => code::STRIKE,
            Capture::String => code::STRING,
            Capture::Strong => code::STRONG,
            Capture::Tag => code::TAG,
            Capture::Type => code::TYPE,
            Capture::Variable => code::VARIABLE,
            _ => return None,
        })
    }
}

impl<H: Highlight, R: Sink + ?Sized> Writer<'_, H, R> {
    /// A code block, highlighted when a grammar knows its language; otherwise as written, with
    /// `code.language.unknown`, or `render.page.failed` when the grammar itself failed.
    pub(super) fn code(&mut self, language: Option<&str>, text: &str) {
        let plain = html! { pre { code { (text) } } };
        let Some(language) = language else { return self.put(plain) };
        match self.source.highlight.highlight(language, text) {
            Ok(runs) => self.put(html! {
                pre {
                    code {
                        @for run in &runs {
                            @match run.capture {
                                Some(capture) => { span class=(Shade(capture).class()) { (run.text) } },
                                None => { (run.text) },
                            }
                        }
                    }
                }
            }),
            Err(Unhighlighted::Unknown) => {
                let notice = Announcement::CodeLanguageUnknown { language: language.to_owned() };
                let subject = self.subject();
                self.record.notice(Notice::new(notice).on(subject));
                self.put(plain);
            }
            Err(failed) => {
                let problem = Problem::RenderPageFailed { reason: Causes(&failed).to_string() };
                let subject = self.subject();
                self.record.diagnose(Diagnostic::new(problem).on(subject));
                self.put(plain);
            }
        }
    }
}

/// An error and each of its causes, as one line: the diagnostic's reason.
#[derive(Clone, Copy, Debug)]
struct Causes<'e>(&'e dyn Error);

impl fmt::Display for Causes<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0)?;
        let mut cause = self.0.source();
        while let Some(error) = cause {
            write!(out, ": {error}")?;
            cause = error.source();
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::{Capture, Shade};

    #[test]
    fn every_capture_has_a_class() {
        for capture in Capture::ALL {
            assert!(Shade(*capture).class().is_some(), "{capture:?} has no class in the code stylesheet");
        }
    }
}
