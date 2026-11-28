//! What `ink help <code>` says about a diagnostic, a notice or a decision code.

use std::fmt;

use diag::{Catalog, Unreadable, Wording};
use model::AnyCode;
use model::decision::Scope;

use super::page::Sheet;
use crate::output::term::{Defs, Line, Role};

/// Why `ink help <word>` has nothing to say.
#[derive(Debug)]
#[non_exhaustive]
pub enum Refusal {
    /// The word is no command, topic or code.
    Topic {
        /// The words, as typed.
        words: Vec<String>,
    },
    /// The catalog this `ink` was built with does not parse.
    Catalog {
        /// Why not.
        source: Unreadable,
    },
}

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Topic { words } => {
                write!(
                    out,
                    "`{}` is no command, no topic (`exit`, `environment`) and no code `ink` knows",
                    words.join(" ")
                )
            }
            Self::Catalog { .. } => write!(out, "the catalog of codes this ink carries cannot be read"),
        }
    }
}

impl std::error::Error for Refusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Catalog { source } => Some(source),
            Self::Topic { .. } => None,
        }
    }
}

/// What a code means, ready to be written.
#[derive(Debug)]
pub(super) enum Meaning {
    /// A diagnostic or a notice, with the catalog's words.
    Said {
        /// The code.
        code: String,
        /// `diagnostic` or `notice`.
        kind: &'static str,
        /// The entry.
        said: Wording,
    },
    /// A decision, which has a name and a subject and no wording of its own.
    Decision {
        /// The code.
        code: AnyCode,
    },
}

impl Meaning {
    /// What `word` is, when it is a code.
    pub(super) fn of(word: &str) -> Result<Self, Refusal> {
        let mut catalog = Catalog::read().map_err(|source| Refusal::Catalog { source })?;
        if let Some(said) = catalog.diagnostic.remove(word) {
            return Ok(Self::Said { code: word.to_owned(), kind: "diagnostic", said });
        }
        if let Some(said) = catalog.notice.remove(word) {
            return Ok(Self::Said { code: word.to_owned(), kind: "notice", said });
        }
        word.parse::<AnyCode>()
            .map(|code| Self::Decision { code })
            .map_err(|_| Refusal::Topic { words: vec![word.to_owned()] })
    }

    /// Writes the meaning into `sheet`.
    pub(super) fn write(&self, sheet: &mut Sheet) {
        match self {
            Self::Said { code, kind, said } => {
                let how = [said.severity.as_deref(), said.fault.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(", ");
                sheet.line(Line::new().strong(Role::Heading, code));
                let about = if how.is_empty() { (*kind).to_owned() } else { format!("A {kind}: {how}.") };
                sheet.paragraph(Line::plain(about));
                let mut defs = Defs::new(2)
                    .row(Line::new().with(Role::Quiet, "It says"), Line::marked(Role::Plain, &said.message));
                if let Some(fix) = &said.fix {
                    defs = defs.row(Line::new().with(Role::Quiet, "The fix"), Line::marked(Role::Plain, fix));
                }
                sheet.section("Meaning", &defs);
            }
            Self::Decision { code } => {
                sheet.line(Line::new().strong(Role::Heading, code.name()));
                let scope = match code.scope() {
                    Scope::Site => "decided once for the site",
                    Scope::File => "decided for each file",
                };
                sheet.paragraph(Line::marked(Role::Plain, &format!("A decision, {scope}. `ink explain` lists what was decided, `ink explain --why {}` replays the ladder that decided it, and `ink pin` writes the value into ink.toml.", code.name())));
            }
        }
    }
}
