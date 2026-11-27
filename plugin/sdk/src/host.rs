//! The host's capabilities, as the native traits a plugin's core is written against.

use std::fmt;

use model::git::{Cloned, Git, Refused, Request};
use model::highlight::{Highlight, Run, Unhighlighted};
use model::{Diagnostic, Notice, Problem, Record, Snapshot};

use crate::bind::ink::host::{diagnostic, git, highlight};
use crate::bind::ink::model::diagnostic as said;
use crate::mirror::{Covered, Malformed, Seen};

/// The host this component runs in: it highlights code through `ink:host/highlight` and records
/// what the plugin says through `ink:host/diagnostic`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Host;

impl Highlight for Host {
    type Error = Failure;

    fn highlight<'c>(&self, language: &str, code: &'c str) -> Result<Vec<Run<'c>>, Unhighlighted<Failure>> {
        match highlight::highlight(language, code) {
            Ok(runs) => {
                Vec::try_from(Covered(code, runs)).map_err(|source| Unhighlighted::Failed(Failure::Runs { source }))
            }
            Err(highlight::Unhighlighted::Unknown) => Err(Unhighlighted::Unknown),
            Err(highlight::Unhighlighted::Failed) => Err(Unhighlighted::Failed(Failure::Grammar)),
        }
    }
}

impl Git for Host {
    type Error = Unfetched;

    fn fetch(&mut self, request: &Request) -> Result<Cloned, Refused<Unfetched>> {
        let cloned = git::fetch(&git::Request::from(request)).map_err(Self::refused)?;
        Cloned::try_from(cloned).map_err(|source| Refused::Failed(Unfetched::Malformed { source }))
    }

    fn snapshot(&mut self, cloned: &Cloned) -> Result<Snapshot, Refused<Unfetched>> {
        let snapshot = git::read(&git::Cloned::from(cloned)).map_err(Self::refused)?;
        Snapshot::try_from(snapshot).map_err(|source| Refused::Failed(Unfetched::Malformed { source }))
    }
}

impl Host {
    /// The host's refusal as the model's; a case the model refuses is a failure.
    fn refused(refusal: git::Refusal) -> Refused<Unfetched> {
        Refused::try_from(Seen(refusal, Unfetched::Said))
            .unwrap_or_else(|source| Refused::Failed(Unfetched::Malformed { source }))
    }
}

impl Record for Host {
    /// Sends `diagnostic` to the host; one the boundary cannot carry is sent as
    /// `plugin.value.unmirrored`, so it is never lost unsaid.
    fn diagnose(&mut self, diagnostic: Diagnostic) {
        diagnostic::emit(&Self::carried(&diagnostic));
    }

    fn notice(&mut self, notice: Notice) {
        match said::Notice::try_from(&notice) {
            Ok(notice) => diagnostic::announce(&notice),
            Err(unmirrored) => diagnostic::emit(&Self::carried(&Diagnostic::new(Problem::PluginValueUnmirrored {
                reason: unmirrored.to_string(),
            }))),
        }
    }
}

impl Host {
    /// `diagnostic` in WIT, or `plugin.value.unmirrored` saying why it cannot be.
    pub fn carried(diagnostic: &Diagnostic) -> said::Diagnostic {
        said::Diagnostic::try_from(diagnostic).unwrap_or_else(|unmirrored| said::Diagnostic {
            chain: vec![said::Said {
                problem: said::Coded {
                    code: "plugin.value.unmirrored".to_owned(),
                    fields: vec![said::Field::Text(unmirrored.to_string())],
                },
                subject: None,
                span: None,
            }],
        })
    }
}

/// A fetch or a read that failed with no case of its own, as the plugin sees it.
#[derive(Debug)]
#[non_exhaustive]
pub enum Unfetched {
    /// The host failed and has said why through `ink:host/diagnostic`.
    Said,
    /// The host sent a value the model refuses.
    Malformed {
        /// Why.
        source: Malformed,
    },
}

impl fmt::Display for Unfetched {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Said => "the host failed, and has said why",
            Self::Malformed { .. } => "the host sent a value the model refuses",
        })
    }
}

impl std::error::Error for Unfetched {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Said => None,
            Self::Malformed { source } => Some(source),
        }
    }
}

/// Why code in a language the host knows was not highlighted.
#[derive(Debug)]
#[non_exhaustive]
pub enum Failure {
    /// The host's grammar failed.
    Grammar,
    /// The host's runs did not cover the code.
    Runs {
        /// How.
        source: Malformed,
    },
}

impl fmt::Display for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Grammar => "the host's grammar failed",
            Self::Runs { .. } => "the host's runs do not cover the code",
        })
    }
}

impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Grammar => None,
            Self::Runs { source } => Some(source),
        }
    }
}
