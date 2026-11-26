//! The one sink of interpreting: every decision, erased and indexed by code and subject so later
//! stages read it back typed, and every diagnostic and notice, in the order said. Nothing is
//! returned on the side and nothing is printed.

use std::collections::BTreeMap;

use model::code::{AnyCode, AnyDecision, Code};
use model::decision::Decision;
use model::{Diagnostic, Notice, Severity, Subject};

use crate::error::Error;
use crate::interpret::decide::Explain;

/// What interpreting has said so far.
#[derive(Debug, Default)]
pub(crate) struct Journal {
    decisions: Vec<AnyDecision>,
    index: BTreeMap<(AnyCode, Subject), usize>,
    diagnostics: Vec<Diagnostic>,
    notices: Vec<Notice>,
}

impl Journal {
    /// Records `decided` and the notice its code says a default calls for, and gives back its
    /// value. A ladder that could not decide, or a second decision of one code on one subject,
    /// is a bug, recorded as one.
    pub(crate) fn record<C: Explain>(&mut self, decided: Result<Decision<C>, Error>) -> Option<C::Value> {
        let decision = match decided {
            Ok(decision) => decision,
            Err(error) => {
                self.fail(error);
                return None;
            }
        };
        let key = (C::CODE, decision.subject.clone().into());
        if self.index.contains_key(&key) {
            let (code, subject) = key;
            self.fail(Error::Repeated { code, subject });
            return None;
        }
        if let Some(announcement) = C::notice(&decision) {
            self.notices.push(Notice::new(announcement).on(key.1.clone()));
        }
        let value = decision.value.clone();
        self.index.insert(key, self.decisions.len());
        self.decisions.push(decision.into());
        Some(value)
    }

    /// The decision of `C` on `subject`.
    pub(crate) fn decision<C: Code>(&self, subject: &C::Subject) -> Option<&Decision<C>> {
        let at = self.index.get(&(C::CODE, subject.clone().into()))?;
        self.decisions.get(*at)?.get::<C>()
    }

    /// The value decided for `C` on `subject`.
    pub(crate) fn value<C: Code>(&self, subject: &C::Subject) -> Option<&C::Value> {
        self.decision::<C>(subject).map(|decision| &decision.value)
    }

    /// The value decided for `C` on `subject`, or the bug of it missing, recorded.
    pub(crate) fn needed<C: Code>(&mut self, subject: &C::Subject) -> Option<C::Value> {
        let value = self.value::<C>(subject).cloned();
        if value.is_none() {
            self.fail(Error::Missing { code: C::CODE, subject: subject.clone().into() });
        }
        value
    }

    /// Records a problem.
    pub(crate) fn diagnose(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Records an announcement.
    pub(crate) fn notice(&mut self, notice: Notice) {
        self.notices.push(notice);
    }

    /// Records a rule of the engine's own that did not hold.
    pub(crate) fn fail(&mut self, error: Error) {
        self.diagnose(error.into());
    }

    /// Whether an error stops the build: all or nothing (open.md Q6).
    pub(crate) fn stops(&self) -> bool {
        self.diagnostics.iter().any(|diagnostic| diagnostic.severity() == Severity::Error)
    }

    /// The decisions, diagnostics and notices, in the order recorded.
    pub(crate) fn into_parts(self) -> (Vec<AnyDecision>, Vec<Diagnostic>, Vec<Notice>) {
        (self.decisions, self.diagnostics, self.notices)
    }
}
