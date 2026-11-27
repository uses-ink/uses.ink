//! The private-source guard (decision 0042): in a vault or a private source, a file is published
//! only by what the writer stated, a pin, a pattern or a frontmatter key, or, for a plain file,
//! by the stated page that uses it. By the ladders this cannot fail; the guard is that rule
//! written down, so a bug in a ladder refuses the build instead of publishing a page.

use model::code::{Code, Publish};
use model::decision::{Decision, File};
use model::{Basis, Diagnostic, Evidence, Problem, Subject};

use crate::interpret::journal::Journal;
use crate::interpret::page::Selection;
use crate::interpret::stage::Stage;

/// Refuses publication no stated selection made, when the site selects pages.
#[derive(Debug)]
pub(crate) struct Guard {
    pub(crate) selection: Selection,
}

impl Stage<()> for Guard {
    type Out = ();

    fn run(&self, (): (), journal: &mut Journal) {
        if self.selection == Selection::Implicit {
            return;
        }
        let refused: Vec<Subject> = journal
            .all::<Publish>()
            .filter(|decision| decision.value && !Self::stated(decision, journal))
            .map(|decision| decision.subject.clone().into())
            .collect();
        for subject in refused {
            journal.diagnose(Diagnostic::new(Problem::PagePublishUnstated { decision: Publish::CODE }).on(subject));
        }
    }
}

impl Guard {
    /// Whether `decision` publishes by what the writer said: directly, or through the page
    /// that uses the file, when that page is published by what the writer said.
    fn stated(decision: &Decision<Publish>, journal: &Journal) -> bool {
        match &decision.evidence {
            evidence if evidence.basis() == Basis::Stated => true,
            Evidence::Marker { path } if *path != decision.subject.0 => journal
                .decision::<Publish>(&File(path.clone()))
                .is_some_and(|user| user.value && user.evidence.basis() == Basis::Stated),
            _ => false,
        }
    }
}

#[cfg(test)]
mod test {
    use model::{ContentPath, DefaultReason, Span};

    use super::*;

    fn published(path: &str, evidence: Evidence) -> Decision<Publish> {
        Decision::new(File(ContentPath::parse(path).unwrap()), true, evidence)
    }

    fn path(text: &str) -> ContentPath {
        ContentPath::parse(text).unwrap()
    }

    fn refused(selection: Selection, decisions: Vec<Decision<Publish>>) -> Vec<Option<Subject>> {
        let mut journal = Journal::default();
        for decision in decisions {
            journal.record(Ok(decision));
        }
        Guard { selection }.run((), &mut journal);
        let (_, diagnostics, _, _) = journal.into_parts();
        diagnostics.into_iter().filter(|d| d.code() == "page.publish.unstated").map(|d| d.subject).collect()
    }

    #[test]
    fn a_private_source_refuses_what_no_stated_selection_published() {
        let front = Evidence::Frontmatter { path: path("a.md"), span: Span::new(0, 1).unwrap() };
        let decisions = || {
            vec![
                published("a.md", front.clone()),
                published("a.png", Evidence::Marker { path: path("a.md") }),
                published("b.md", Evidence::Default { reason: DefaultReason::NothingMatched }),
                published("b.png", Evidence::Marker { path: path("b.md") }),
                published("c.md", Evidence::Heading { path: path("c.md"), span: Span::new(0, 1).unwrap() }),
            ]
        };
        let refused_here = refused(Selection::Explicit, decisions());
        let file = |text| Some(Subject::File(path(text)));
        assert_eq!(refused_here, [file("b.md"), file("b.png"), file("c.md")]);
        assert!(refused(Selection::Implicit, decisions()).is_empty(), "a public source publishes by default");
    }
}
