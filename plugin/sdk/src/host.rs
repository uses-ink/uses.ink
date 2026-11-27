//! The host's capabilities, as the native traits a plugin's core is written against.

use std::fmt;

use model::highlight::{Highlight, Run, Unhighlighted};
use model::{Diagnostic, Notice, Problem, Record};

use crate::bind::ink::host::{diagnostic, highlight};
use crate::bind::ink::model::diagnostic as said;
use crate::mirror::{Covered, Malformed};

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
