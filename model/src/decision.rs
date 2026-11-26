//! Decisions: one recorded inference each, with its subject, its value, the evidence that chose
//! it and the alternatives it beat. The `Site` is built only from decisions.
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
    /// Every value that also testified and lost, in precedence order.
    pub rejected: Vec<Alternative<C::Value>>,
}

impl<C: Code> Decision<C> {
    /// A decision that rejected nothing.
    pub fn new(subject: C::Subject, value: C::Value, evidence: Evidence) -> Self {
        Self { subject, value, evidence, rejected: Vec::new() }
    }

    /// The same decision, recording `alternative` as rejected after the ones it has.
    #[must_use]
    pub fn rejecting(mut self, alternative: Alternative<C::Value>) -> Self {
        self.rejected.push(alternative);
        self
    }
}

/// A value of type `V` a decision considered and did not take.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Alternative<V> {
    /// The value it would have been.
    pub value: V,
    /// What testified to it.
    pub evidence: Evidence,
    /// Why it lost.
    pub because: Rejection,
}

impl<V> Alternative<V> {
    /// `value`, testified to by `evidence`, lost `because`.
    pub fn new(value: V, evidence: Evidence, because: Rejection) -> Self {
        Self { value, evidence, because }
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
    /// A file or folder whose presence says something, such as `.obsidian/`.
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

/// Why an alternative lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Rejection {
    /// Evidence earlier in the decision's precedence chose.
    Outranked,
    /// The value was only a bound.
    OnlyBound,
}
