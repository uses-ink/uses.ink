//! The docs checks, `cargo xtask check docs` (the first step of its stage), and `cargo xtask
//! docs`, which writes the decisions index. Each check is a [`Check`] over the [`Corpus`], the
//! text of the tree read once: relative links resolve, citations of decisions resolve, backticked
//! crate paths exist, the front matter of every decision is valid and the index is current. What
//! they find is a [`Fault`] each, all listed in one failure. The fan-out rule stays in `layout`
//! and only reports.

mod cite;
mod corpus;
mod crates;
mod decision;
mod fault;
mod front;
mod index;
mod link;
mod number;

use std::fmt;
use std::path::Path;

use crate::check::Step;
use crate::error::Error;
use crate::report::{Line, Report};

pub(crate) use fault::Fault;
pub(crate) use index::Index;
pub(crate) use number::Number;

use cite::Cites;
use corpus::Corpus;
use crates::Crates;
use decision::{Fronts, Gaps};
use link::Links;

/// One check over the whole corpus. A new check is a new type.
pub(crate) trait Check {
    /// Adds every fault it finds to `faults`.
    fn check(&self, corpus: &Corpus, faults: &mut Vec<Fault>);
}

/// A list of checks, each run in order. Implemented for tuples of [`Check`]s.
pub(crate) trait Checks {
    fn check(&self, corpus: &Corpus, faults: &mut Vec<Fault>);
}

tuples! { Checks: Check, fn check(&self, corpus: &Corpus, faults: &mut Vec<Fault>) { |check| check.check(corpus, faults) } }

/// The docs checks, as a step of the gate.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Prose;

/// Every check, as a tuple.
type Every = (Links, Cites, Crates, Fronts, Gaps, Index);

impl Prose {
    /// Every check, in the order their faults are listed.
    const CHECKS: Every = (Links, Cites, Crates, Fronts, Gaps, Index);
}

impl Step for Prose {
    fn run(&self, root: &Path, report: &mut Report) {
        match Corpus::read(root) {
            Ok(corpus) => {
                let mut faults = Vec::new();
                Self::CHECKS.check(&corpus, &mut faults);
                if faults.is_empty() {
                    report.line(Line::Prose);
                } else {
                    report.fail(Error::Docs(faults));
                }
            }
            Err(error) => report.fail(error),
        }
    }
}

impl fmt::Display for Prose {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the docs checks (links, citations, crate paths, decisions)")
    }
}
