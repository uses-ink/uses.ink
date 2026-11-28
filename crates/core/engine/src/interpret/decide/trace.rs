//! A ladder replayed for one decision, rung by rung: what each witness said, including the
//! witnesses that found nothing, and which testimony was chosen. `ink explain --why` asks for
//! one; a normal run keeps only the decision (decision 0042). Nothing here reaches the model or
//! the record.

use std::fmt;

use model::{AnyCode, AnyDecision, Subject};
use serde::Serialize;

/// One decision's ladder as it ran.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Trace {
    /// The code decided.
    pub code: AnyCode,
    /// What it was decided about.
    pub subject: Subject,
    /// Every witness of the ladder, first first.
    pub rungs: Vec<Rung>,
}

impl Trace {
    /// Where the chosen testimony stands: its rung's place, counted from 1, and the
    /// testimony itself. `None` when nothing was chosen, which is the engine's bug.
    pub fn chosen(&self) -> Option<(usize, &Heard)> {
        self.rungs
            .iter()
            .enumerate()
            .find_map(|(at, rung)| rung.heard.iter().find(|heard| heard.chosen).map(|heard| (at + 1, heard)))
    }
}

/// One witness of a ladder and what it said.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Rung {
    /// The witness.
    pub witness: Witnessed,
    /// What it said, in order; empty when it found nothing.
    pub heard: Vec<Heard>,
}

/// One testimony a witness gave.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Heard {
    /// The value and its evidence, as the decision it would have been.
    pub testimony: AnyDecision,
    /// Whether it was a value or only a bound.
    pub standing: Standing,
    /// Whether it is the decision.
    pub chosen: bool,
}

/// Whether a testified value can be chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Standing {
    /// It can.
    Value,
    /// It is only a bound on the value (`Known::NotAfter`), never chosen.
    Bound,
}

/// A witness, as a person names it. Each witness type says which it is through [`Named`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Witnessed {
    /// A pin in `ink.toml`.
    Pin,
    /// The page's frontmatter key for the code.
    Front,
    /// What the page's text adds to its frontmatter: its inline tags.
    Inline,
    /// The page's `draft` key.
    Draft,
    /// The page's first level-1 heading.
    Heading,
    /// The page's first paragraph.
    Body,
    /// The file's name, read one way.
    Name(Reading),
    /// A fact of the source's history.
    History(Fact),
    /// A folder whose presence says something.
    Folder(Marker),
    /// An `include` or `exclude` pattern.
    Pattern(List),
    /// The home page's decision of another code.
    Derived(AnyCode),
    /// A file at the top named as a home page.
    Index,
    /// A published page that uses the file.
    Used,
    /// The default.
    Default,
}

/// How a file's name is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Reading {
    /// A `YYYY-MM-DD` prefix.
    Dated,
    /// The stem after the date.
    Stem,
    /// A file about the repository, such as `LICENSE`.
    Repository,
}

/// A fact of the history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Fact {
    /// When the file was created.
    Created,
    /// When it last changed.
    Changed,
}

/// A folder marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Marker {
    /// A top folder of posts.
    Posts,
    /// A drafts folder, or a name starting with `_`.
    Drafts,
    /// A dotfile, a dot folder or vendored files.
    Hidden,
    /// A vault's templates folder.
    Templates,
    /// A vault's `.obsidian/`.
    Obsidian,
    /// A root named `docs`.
    Docs,
    /// The one content folder.
    Content,
}

/// A list of patterns in `ink.toml`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum List {
    /// `include`.
    Include,
    /// `exclude`.
    Exclude,
}

impl fmt::Display for Witnessed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pin => out.write_str("ink.toml"),
            Self::Front => out.write_str("front matter"),
            Self::Inline => out.write_str("tags in the text"),
            Self::Draft => out.write_str("draft key"),
            Self::Heading => out.write_str("first heading"),
            Self::Body => out.write_str("first paragraph"),
            Self::Name(Reading::Dated) => out.write_str("dated file name"),
            Self::Name(Reading::Stem) => out.write_str("file name"),
            Self::Name(Reading::Repository) => out.write_str("repository file"),
            Self::History(Fact::Created) => out.write_str("history, created"),
            Self::History(Fact::Changed) => out.write_str("history, changed"),
            Self::Folder(Marker::Posts) => out.write_str("posts folder"),
            Self::Folder(Marker::Drafts) => out.write_str("drafts folder"),
            Self::Folder(Marker::Hidden) => out.write_str("hidden folder"),
            Self::Folder(Marker::Templates) => out.write_str("templates folder"),
            Self::Folder(Marker::Obsidian) => out.write_str(".obsidian folder"),
            Self::Folder(Marker::Docs) => out.write_str("docs folder"),
            Self::Folder(Marker::Content) => out.write_str("content folder"),
            Self::Pattern(List::Include) => out.write_str("include"),
            Self::Pattern(List::Exclude) => out.write_str("exclude"),
            Self::Derived(code) => write!(out, "home's {code}"),
            Self::Index => out.write_str("home file name"),
            Self::Used => out.write_str("used by a page"),
            Self::Default => out.write_str("default"),
        }
    }
}

/// A witness type, named once: what a trace calls its rung.
pub(crate) trait Named {
    /// Its name.
    const NAME: Witnessed;
}
