//! Decisions: one recorded inference each, with its subject, its value, the evidence that chose
//! it and the alternatives it beat. The `Site` is built only from decisions.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{Author, ContentPath, Language, Layout, PageKind, Provenance, Slug, Span, Tag, When};

/// What a decision is about.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Subject {
    Site,
    File(ContentPath),
}

impl fmt::Display for Subject {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Site => out.write_str("the site"),
            Self::File(path) => write!(out, "{path}"),
        }
    }
}

/// Where a site's content starts in the snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Root {
    /// The top of the snapshot.
    Top,
    Folder(ContentPath),
}

/// A decided value, one variant per decision code. The variant is the code; there is no code
/// apart from the value to get out of step with it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Decided {
    Root(Root),
    Layout(Layout),
    Ignored(bool),
    /// The home page's file, or `None` for a generated listing.
    Home(Option<ContentPath>),
    Kind(PageKind),
    Publish(bool),
    Title(String),
    Date(Option<When>),
    Updated(Option<When>),
    Slug(Slug),
    Tags(Vec<Tag>),
    Nav(Vec<ContentPath>),
    /// A page's summary for feeds, listings and `og:description`, or `None` when it has no text.
    Summary(Option<String>),
    /// The site's name, shown in every page's title and its feeds.
    SiteTitle(String),
    /// One line saying what the site is, or `None`.
    Description(Option<String>),
    /// Who wrote the site, first author first.
    Authors(Vec<Author>),
    /// The language of the site's pages.
    Language(Language),
}

impl Decided {
    pub const fn code(&self) -> DecisionCode {
        match self {
            Self::Root(_) => DecisionCode::Root,
            Self::Layout(_) => DecisionCode::Layout,
            Self::Ignored(_) => DecisionCode::Ignored,
            Self::Home(_) => DecisionCode::Home,
            Self::Kind(_) => DecisionCode::Kind,
            Self::Publish(_) => DecisionCode::Publish,
            Self::Title(_) => DecisionCode::Title,
            Self::Date(_) => DecisionCode::Date,
            Self::Updated(_) => DecisionCode::Updated,
            Self::Slug(_) => DecisionCode::Slug,
            Self::Tags(_) => DecisionCode::Tags,
            Self::Nav(_) => DecisionCode::Nav,
            Self::Summary(_) => DecisionCode::Summary,
            Self::SiteTitle(_) => DecisionCode::SiteTitle,
            Self::Description(_) => DecisionCode::Description,
            Self::Authors(_) => DecisionCode::Authors,
            Self::Language(_) => DecisionCode::Language,
        }
    }
}

/// The code of a decision, without its value. Its name is the `ink.toml` path that pins it
/// (decision 0018): `site.home` is `[site] home`, `page.title` on a file is
/// `[page."<path>"] title`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(into = "&'static str", try_from = "String")]
#[non_exhaustive]
pub enum DecisionCode {
    Root,
    Layout,
    Ignored,
    Home,
    Kind,
    Publish,
    Title,
    Date,
    Updated,
    Slug,
    Tags,
    Nav,
    Summary,
    SiteTitle,
    Description,
    Authors,
    Language,
}

impl DecisionCode {
    /// Every code, in the order the inferrers decide them (architecture.md section 9).
    pub const ALL: &'static [Self] = &[
        Self::Root,
        Self::Layout,
        Self::Ignored,
        Self::Kind,
        Self::Publish,
        Self::Title,
        Self::Summary,
        Self::Date,
        Self::Updated,
        Self::Slug,
        Self::Tags,
        Self::Home,
        Self::Nav,
        Self::SiteTitle,
        Self::Description,
        Self::Authors,
        Self::Language,
    ];

    /// `table.key`, the pin's path in `ink.toml`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Root => "site.root",
            Self::Layout => "site.layout",
            Self::Ignored => "file.ignore",
            Self::Home => "site.home",
            Self::Kind => "page.kind",
            Self::Publish => "page.publish",
            Self::Title => "page.title",
            Self::Date => "page.date",
            Self::Updated => "page.updated",
            Self::Slug => "page.slug",
            Self::Tags => "page.tags",
            Self::Nav => "site.nav",
            Self::Summary => "page.summary",
            Self::SiteTitle => "site.title",
            Self::Description => "site.description",
            Self::Authors => "site.authors",
            Self::Language => "site.language",
        }
    }

    /// Whether it is decided once for the site rather than once per file.
    pub const fn is_sitewide(self) -> bool {
        matches!(
            self,
            Self::Root
                | Self::Layout
                | Self::Home
                | Self::Nav
                | Self::SiteTitle
                | Self::Description
                | Self::Authors
                | Self::Language
        )
    }
}

impl fmt::Display for DecisionCode {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

/// A name that is no decision code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownDecision {
    pub name: String,
}

impl fmt::Display for UnknownDecision {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`{}` is no decision code", self.name)
    }
}

impl std::error::Error for UnknownDecision {}

impl FromStr for DecisionCode {
    type Err = UnknownDecision;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|code| code.name() == name)
            .ok_or_else(|| UnknownDecision { name: name.to_owned() })
    }
}

impl TryFrom<String> for DecisionCode {
    type Error = UnknownDecision;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        name.parse()
    }
}

impl From<DecisionCode> for &'static str {
    fn from(code: DecisionCode) -> Self {
        code.name()
    }
}

/// Why a decision chose what it chose.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Evidence {
    Frontmatter {
        path: ContentPath,
        span: Span,
    },
    Filename {
        path: ContentPath,
    },
    Heading {
        path: ContentPath,
        span: Span,
    },
    History {
        from: Provenance,
    },
    /// A page's own text, such as its first paragraph as its summary.
    Body {
        path: ContentPath,
    },
    /// A file or folder whose presence says something, such as `.obsidian/`.
    Marker {
        path: ContentPath,
    },
    /// A pin in `ink.toml`: "you said so".
    Config {
        span: Span,
    },
    Default {
        reason: DefaultReason,
    },
}

/// Why a default was taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
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

/// A value a decision considered and did not take.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
pub struct Alternative {
    pub decided: Decided,
    pub evidence: Evidence,
    pub because: Rejection,
}

impl Alternative {
    pub fn new(decided: Decided, evidence: Evidence, because: Rejection) -> Self {
        Self { decided, evidence, because }
    }
}

/// Why an alternative lost.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Rejection {
    /// Evidence earlier in the decision's precedence chose.
    Outranked,
    /// The value was only a bound.
    OnlyBound,
}

/// One recorded inference. Its pin is derived from its code and subject, never stored.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
pub struct Decision {
    pub decided: Decided,
    pub subject: Subject,
    pub evidence: Evidence,
    pub rejected: Vec<Alternative>,
}

impl Decision {
    pub fn new(decided: Decided, subject: Subject, evidence: Evidence) -> Self {
        Self { decided, subject, evidence, rejected: Vec::new() }
    }

    /// The same decision, recording `alternative` as rejected.
    #[must_use]
    pub fn rejecting(mut self, alternative: Alternative) -> Self {
        self.rejected.push(alternative);
        self
    }

    pub const fn code(&self) -> DecisionCode {
        self.decided.code()
    }
}

#[cfg(test)]
mod test {
    use super::DecisionCode;

    #[test]
    fn every_code_is_listed_once_and_parses_back() {
        for code in DecisionCode::ALL {
            assert_eq!(code.name().parse::<DecisionCode>(), Ok(*code));
            assert_eq!(DecisionCode::ALL.iter().filter(|c| *c == code).count(), 1);
        }
        assert!("site.nope".parse::<DecisionCode>().is_err());
    }
}
