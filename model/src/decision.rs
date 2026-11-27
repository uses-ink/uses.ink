//! Decisions: one recorded inference each, with its subject, its value and the one piece of
//! evidence that chose it (decision 0042). The `Site` is built only from decisions.
//!
//! A [`Decision<C>`] is typed by its code `C` (see [`code`](crate::code)): a `page.title`
//! decision holds a `String` about a [`File`], and cannot hold a date or be about the site.
//! Storage and the wire hold the erased [`AnyDecision`](crate::AnyDecision).

use serde::{Deserialize, Serialize};

use crate::code::Code;
use crate::{ContentPath, Provenance, Span};

mod subject;

pub use subject::{File, Mismatch, Scope, Site, Subject};

/// One recorded inference of code `C`. Its pin is derived from its code and subject, never
/// stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
#[non_exhaustive]
pub struct Decision<C: Code> {
    /// What it is about: the site, or one file.
    pub subject: C::Subject,
    /// The value chosen.
    pub value: C::Value,
    /// Why this value.
    pub evidence: Evidence,
}

impl<C: Code> Decision<C> {
    /// `value` about `subject`, chosen because of `evidence`.
    pub fn new(subject: C::Subject, value: C::Value, evidence: Evidence) -> Self {
        Self { subject, value, evidence }
    }
}

/// Why a decision chose what it chose.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Evidence {
    /// A frontmatter key of the file, where it is written.
    Frontmatter {
        /// The file.
        path: ContentPath,
        /// The value's place in the file.
        span: Span,
    },
    /// The file's name, such as a date prefix.
    Filename {
        /// The file.
        path: ContentPath,
    },
    /// The file's first level-1 heading.
    Heading {
        /// The file.
        path: ContentPath,
        /// The heading's place in the file.
        span: Span,
    },
    /// A value found in a page's text, where it is written: an inline `#tag`.
    Inline {
        /// The file.
        path: ContentPath,
        /// The value's place in the file.
        span: Span,
    },
    /// A fact from the source's history.
    History {
        /// Where the fact came from.
        from: Provenance,
    },
    /// A page's own text, such as its first paragraph as its summary.
    Body {
        /// The file.
        path: ContentPath,
    },
    /// A file or folder whose presence says something: `.obsidian/` of a vault, `posts/` of the
    /// posts it holds, the published page that uses an asset.
    Marker {
        /// The file or folder.
        path: ContentPath,
    },
    /// A pin in `ink.toml`: "you said so".
    Config {
        /// The pin's place in `ink.toml`.
        span: Span,
    },
    /// Nothing testified, so the default was taken.
    Default {
        /// Why nothing did.
        reason: DefaultReason,
    },
}

/// Why a default was taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum DefaultReason {
    /// No input of the decision's table matched.
    NothingMatched,
    /// The only fact was a bound, not a value (`Known::NotAfter`).
    OnlyBound,
    /// A vault or private source publishes nothing until it is selected (decision 0017).
    NotSelected,
}

/// Who said a decision's value: the writer, a reading of something else, or no one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Basis {
    /// The writer said it: a pin or pattern in `ink.toml`, a frontmatter key.
    Stated,
    /// It was read from something else: a heading, a file name, the text, a folder, the
    /// history.
    Inferred,
    /// Nothing said anything, so the default was taken.
    Assumed,
}

impl Evidence {
    /// Who said the value this evidence chose.
    pub const fn basis(&self) -> Basis {
        match self {
            Self::Config { .. } | Self::Frontmatter { .. } => Basis::Stated,
            Self::Filename { .. }
            | Self::Heading { .. }
            | Self::Inline { .. }
            | Self::History { .. }
            | Self::Body { .. }
            | Self::Marker { .. } => Basis::Inferred,
            Self::Default { .. } => Basis::Assumed,
        }
    }
}
