//! The stage line: a past-tense verb in a gutter, then a sentence (`docs/cli.md` sections 6 and 7).
//! On stderr it is text; with `--json` a finished stage is a `uses.progress/v1` event.

use std::fmt;

use model::Version;

use crate::output::Tally;
use crate::term::{Beat, Doc, Elapsed, Gutter, Line, Phase, Render, Role, Strength};
use diag::Short;

/// What a stage line starts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Verb {
    Read,
    Understood,
    Rendered,
    Changes,
    Built,
    Checked,
    Selected,
    Deselected,
    WouldSelect,
    WouldDeselect,
    Unchanged,
    Pinned,
    WouldPin,
}

impl Verb {
    /// The word, as it is written in the gutter.
    pub(crate) const fn word(self) -> &'static str {
        match self {
            Self::Read => "Read",
            Self::Understood => "Understood",
            Self::Rendered => "Rendered",
            Self::Changes => "Changes",
            Self::Built => "Built",
            Self::Checked => "Checked",
            Self::Selected => "Selected",
            Self::Deselected => "Deselected",
            Self::WouldSelect => "Would select",
            Self::WouldDeselect => "Would deselect",
            Self::Unchanged => "Unchanged",
            Self::Pinned => "Pinned",
            Self::WouldPin => "Would pin",
        }
    }

    /// The stage the verb ends, for the progress event of a finished stage.
    const fn phase(self) -> Option<Phase> {
        match self {
            Self::Read => Some(Phase::Reading),
            Self::Understood => Some(Phase::Understanding),
            Self::Rendered => Some(Phase::Rendering),
            Self::Built => Some(Phase::Writing),
            Self::Checked => Some(Phase::Checking),
            _ => None,
        }
    }
}

/// One stage line.
#[derive(Clone, Debug)]
pub(crate) struct StageLine {
    verb: Verb,
    role: Role,
    strength: Strength,
    text: Line,
    took: Option<Elapsed>,
}

impl StageLine {
    /// `verb`, then `text`, in the colour of good news.
    pub(crate) fn new(verb: Verb, text: impl fmt::Display) -> Self {
        Self { verb, role: Role::Good, strength: Strength::Normal, text: Line::plain(text.to_string()), took: None }
    }

    /// The closing line of a command: bold, and in the colour of the worst thing it counted.
    #[must_use]
    pub(crate) fn closing(mut self, tally: Tally) -> Self {
        self.strength = Strength::Strong;
        self.role = if tally.errors > 0 {
            Role::Bad
        } else if tally.warnings > 0 {
            Role::Warn
        } else {
            Role::Good
        };
        self
    }

    /// The line says how long the stage took.
    #[must_use]
    pub(crate) fn took(mut self, elapsed: Elapsed) -> Self {
        self.took = Some(elapsed);
        self
    }

    /// The `uses.progress/v1` event of the finished stage, when the verb ends one.
    pub(crate) fn beat(&self) -> Option<Beat> {
        self.verb.phase().map(|phase| Beat::finished(phase, self.text.text()))
    }
}

impl Render for StageLine {
    fn render(&self, into: &mut Doc) {
        let mut text = self.text.clone();
        if let Some(elapsed) = self.took {
            text = text.with(Role::Quiet, format!(", {elapsed}"));
        }
        Gutter { word: self.verb.word().to_owned(), role: self.role, strength: self.strength, text }.render(into);
    }
}

/// A count and what is counted: `1 page`, `61 pages`. Every noun here is regular.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Plural(pub(crate) usize, pub(crate) &'static str);

impl fmt::Display for Plural {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            1 => write!(out, "1 {}", self.1),
            count => write!(out, "{count} {}s", self.1),
        }
    }
}

/// What a command counted: `1 error, 2 warnings`, or `no errors, no warnings`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Counted(pub(crate) Tally);

impl fmt::Display for Counted {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = |count: usize, noun: &'static str| match count {
            0 => format!("no {noun}s"),
            count => Plural(count, noun).to_string(),
        };
        write!(out, "{}, {}", count(self.0.errors, "error"), count(self.0.warnings, "warning"))
    }
}

/// A version as a stage says where it read: `git 9f3c2a1b4d7e`, `the files on disk`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Sourced<'a>(pub(crate) &'a Version);

impl fmt::Display for Sourced<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Version::GitCommit(commit) => write!(out, "git {}", Short(commit)),
            Version::Folder { .. } => write!(out, "the files on disk"),
            Version::Upload(id) => write!(out, "upload {id}"),
            _ => write!(out, "a version this ink cannot show"),
        }
    }
}
