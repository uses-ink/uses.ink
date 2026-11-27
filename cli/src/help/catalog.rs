//! What `ink help <code>` says about a diagnostic, a notice or a decision code.

use std::collections::BTreeMap;
use std::fmt;

use model::AnyCode;
use model::decision::Scope;
use serde::Deserialize;

use super::page::{Page, Row};
use super::paint::Paint;

/// The catalog the diagnostics and notices are generated from, read as it was at build time.
const CATALOG: &str = include_str!("../../../diag/catalog.toml");

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
        source: toml::de::Error,
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

/// One entry of the catalog.
#[derive(Debug, Deserialize)]
pub(super) struct Said {
    severity: Option<String>,
    fault: Option<String>,
    message: String,
    fix: Option<String>,
}

/// The tables of the catalog this help reads.
#[derive(Debug, Default, Deserialize)]
struct Catalog {
    #[serde(default)]
    diagnostic: BTreeMap<String, Said>,
    #[serde(default)]
    notice: BTreeMap<String, Said>,
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
        said: Said,
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
        let mut catalog: Catalog = toml::from_str(CATALOG).map_err(|source| Refusal::Catalog { source })?;
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

    /// Writes the meaning into `page`.
    pub(super) fn write<P: Paint>(&self, page: &mut Page<'_, P>) {
        match self {
            Self::Said { code, kind, said } => {
                let how = [said.severity.as_deref(), said.fault.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(", ");
                page.heading(code);
                let about = if how.is_empty() { (*kind).to_owned() } else { format!("a {kind}: {how}") };
                page.paragraph(&about);
                let mut rows = vec![Row::keyed("It says", said.message.as_str())];
                if let Some(fix) = &said.fix {
                    rows.push(Row::keyed("The fix", fix.as_str()));
                }
                page.gap();
                page.rows(&rows);
            }
            Self::Decision { code } => {
                page.heading(code.name());
                let scope = match code.scope() {
                    Scope::Site => "decided once for the site",
                    _ => "decided for each file",
                };
                page.paragraph(&format!("A decision, {scope}. `ink explain` lists what was decided, `ink explain --why {}` replays the ladder that decided it, and `ink pin` writes the value into ink.toml.", code.name()));
            }
        }
    }
}
