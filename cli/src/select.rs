//! `ink select`: what is published and why, and recording a selection (decisions 0042, open.md
//! Q16 and Q17). In a vault or a private source a page is private until it is selected, by
//! `publish: true` in its frontmatter or an `include` pattern in `ink.toml`; in a public source
//! every page is published unless it says otherwise, and selecting only states it.
//!
//! The listing is the decision `page.publish` and its evidence. A selection is planned whole
//! before anything is written: one refused file stops all of them. It reads the files on disk,
//! since it writes into them.

use std::fmt;
use std::path::{Path, PathBuf};

use config::{Config, Listed, Listing, Pattern};
use engine::Selection;
use format_markdown::{Edit, Publishing};
use model::code::{Ignored, Kind, Publish};
use model::decision::File;
use model::{AnyDecision, ContentPath, Decision, DefaultReason, Evidence};
use serde::Serialize;

use crate::args::{Kind as Reading, Origin, Select};
use crate::error::Error;
use crate::exit::Exit;
use crate::explain::Because;
use crate::output::{Io, Strict};
use crate::report::Texts;
use crate::site::{Loaded, Source};

/// Why a file cannot be selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Unselectable {
    /// It is excluded or ignored, so it is no part of the site.
    Ignored,
    /// It is a plain file, published when a published page uses it.
    Plain,
    /// It is not markdown, so there is no frontmatter this ink edits.
    Format,
}

impl fmt::Display for Unselectable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Ignored => "it is ignored, so it is no part of the site",
            Self::Plain => "it is a plain file, published when a published page uses it",
            Self::Format => "only the frontmatter of a markdown file is edited; `--folder` selects any file",
        })
    }
}

/// What happened, or would happen, to one place a selection is recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Outcome {
    /// `publish: true` was added to the frontmatter.
    Added,
    /// `publish: false` became `publish: true`.
    Replaced,
    /// The frontmatter already says `publish: true`.
    Already,
    /// `publish: true` was removed from the frontmatter.
    Removed,
    /// The frontmatter says `publish: false`, which is no selection and is left as it is.
    Kept,
    /// The frontmatter has no `publish` to remove.
    Absent,
    /// `page.publish` is pinned in `ink.toml`, which outranks the frontmatter: nothing to write.
    Pinned,
    /// The pattern was added to `include`.
    Listed,
    /// The pattern is already in `include`.
    Present,
    /// The pattern was removed from `include`.
    Delisted,
    /// The pattern is not in `include`.
    Unlisted,
}

impl fmt::Display for Outcome {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Added => "added `publish: true` to the frontmatter",
            Self::Replaced => "changed `publish: false` to `publish: true` in the frontmatter",
            Self::Already => "the frontmatter already says `publish: true`",
            Self::Removed => "removed `publish: true` from the frontmatter",
            Self::Kept => "the frontmatter says `publish: false`, which is no selection: left as it is",
            Self::Absent => "the frontmatter has no `publish: true` to remove",
            Self::Pinned => "`page.publish` is pinned in ink.toml, which outranks the frontmatter: nothing written",
            Self::Listed => "added to `include` in ink.toml",
            Self::Present => "`include` in ink.toml already lists it",
            Self::Delisted => "removed from `include` in ink.toml",
            Self::Unlisted => "`include` in ink.toml does not list it",
        })
    }
}

impl From<Listed> for Outcome {
    fn from(listed: Listed) -> Self {
        match listed {
            Listed::Added => Self::Listed,
            Listed::Present => Self::Present,
            Listed::Removed => Self::Delisted,
            _ => Self::Unlisted,
        }
    }
}

/// What a selection records, and where.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "place", rename_all = "kebab-case")]
enum Place {
    /// The frontmatter of one page.
    File { path: ContentPath },
    /// `include` in `ink.toml`, and how many files of the folder the pattern matches.
    Folder { pattern: String, matches: usize },
}

impl fmt::Display for Place {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File { path } => write!(out, "{path}"),
            Self::Folder { pattern, .. } => write!(out, "include {pattern}"),
        }
    }
}

/// One edit a selection makes: where, what it comes to, and the text it writes, if it writes any.
#[derive(Clone, Debug, Serialize)]
struct Step {
    #[serde(flatten)]
    place: Place,
    outcome: Outcome,
    #[serde(skip)]
    text: Option<String>,
}

impl fmt::Display for Step {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{:<32} {}", self.place.to_string(), self.outcome)?;
        match &self.place {
            Place::Folder { matches: 1, .. } => out.write_str(", matches 1 file"),
            Place::Folder { matches, .. } => write!(out, ", matches {matches} files"),
            Place::File { .. } => Ok(()),
        }
    }
}

/// How a selection ended.
#[derive(Clone, Copy, Debug)]
enum Ended {
    Wrote(usize),
    Dry,
    Nothing,
}

impl fmt::Display for Ended {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wrote(1) => out.write_str("wrote 1 file"),
            Self::Wrote(count) => write!(out, "wrote {count} files"),
            Self::Dry => out.write_str("dry run: nothing written"),
            Self::Nothing => out.write_str("nothing to write"),
        }
    }
}

/// What `ink select --json` prints: how pages are selected, each page's `page.publish` as it
/// stood before this command wrote anything, and what the command did.
#[derive(Debug, Serialize)]
struct Selected<'a> {
    schema: &'static str,
    selection: Selection,
    pages: Vec<Page<'a>>,
    steps: &'a [Step],
    written: usize,
}

/// A page's decision `page.publish`, typed, and as the wire holds it (tagged by its code).
#[derive(Clone, Copy, Debug)]
struct Page<'a> {
    decision: &'a Decision<Publish>,
    wire: &'a AnyDecision,
}

impl Serialize for Page<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.wire.serialize(serializer)
    }
}

/// A page's standing: its decision `page.publish`, worded for a person.
#[derive(Clone, Copy, Debug)]
struct Standing<'a> {
    decision: &'a Decision<Publish>,
    config: &'a Config,
}

impl fmt::Display for Standing<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let decision = self.decision;
        match &decision.evidence {
            Evidence::Config { span } => match self.config.include().iter().find(|listed| listed.span == *span) {
                Some(listed) => write!(out, "selected by the `include` pattern `{}` in ink.toml", listed.value),
                None => write!(out, "pinned in ink.toml, {span}"),
            },
            Evidence::Default { reason: DefaultReason::NotSelected } => {
                out.write_str("not selected: a vault or private source keeps a page private until it is selected")
            }
            Evidence::Default { .. } => out.write_str("by default: a public source publishes its pages"),
            Evidence::Frontmatter { path, span } => {
                write!(out, "the frontmatter of {path} says so, {span}")
            }
            other => Because(other).fmt(out),
        }
    }
}

/// One listed page: `  path   why`.
#[derive(Clone, Copy, Debug)]
struct Row<'a>(Standing<'a>);

impl fmt::Display for Row<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "  {:<32} {}", self.0.decision.subject.0, self.0)
    }
}

/// Every page and its decision `page.publish`, in path order.
#[derive(Clone, Copy, Debug)]
struct Pages<'a>(&'a Loaded);

impl<'a> Pages<'a> {
    /// The decisions of the files that are pages, not plain files.
    fn decisions(self) -> Vec<Page<'a>> {
        let kinds: std::collections::BTreeSet<&ContentPath> =
            self.0.understood.decisions.iter().filter_map(|d| d.get::<Kind>()).map(|d| &d.subject.0).collect();
        let decided = self.0.understood.decisions.iter();
        decided
            .filter_map(|wire| wire.get::<Publish>().map(|decision| Page { decision, wire }))
            .filter(|page| kinds.contains(&page.decision.subject.0))
            .collect()
    }

    /// Whether the file is ignored: excluded, hidden or a template.
    fn ignores(self, path: &ContentPath) -> bool {
        self.0.understood.decisions.iter().filter_map(|d| d.get::<Ignored>()).any(|d| d.subject.0 == *path && d.value)
    }

    /// Whether the file is a page.
    fn holds(self, path: &ContentPath) -> bool {
        self.decisions().iter().any(|page| page.decision.subject.0 == *path)
    }
}

/// How the listing words the way pages are selected.
#[derive(Clone, Copy, Debug)]
struct Selecting(Selection);

impl fmt::Display for Selecting {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Selection::Explicit => out.write_str(
                "selection: explicit, a vault or private source: a page is private until it is selected",
            ),
            _ => out.write_str("selection: implicit, a public source: a page is published unless it says otherwise"),
        }
    }
}

/// What the command is asked to do with the places it was given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verb {
    Select,
    Remove,
}

impl Select {
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        let origin = Origin { source: Some(Reading::Folder), rev: None };
        let loaded = Source::new(&self.dir, &origin).load()?;
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let understood = &loaded.understood;
        let tally = io.said(&understood.diagnostics, &understood.notices, texts)?;
        let pages = Pages(&loaded).decisions();
        let steps = self.plan(&loaded)?;
        let ended = self.write(&steps)?;
        if steps.is_empty() {
            io.line(Selecting(understood.selection))?;
            Self::list(io, &pages, &loaded.config)?;
        }
        for step in &steps {
            io.line(step)?;
        }
        if !steps.is_empty() {
            io.line(ended)?;
        }
        let written = if let Ended::Wrote(count) = ended { count } else { 0 };
        io.json(&Selected { schema: "uses.select/v1", selection: understood.selection, pages, steps: &steps, written })?;
        Ok(tally.exit(Strict::Off))
    }

    /// The pages that are published, then those that are not, each with why.
    fn list(io: &mut Io<'_>, pages: &[Page<'_>], config: &Config) -> Result<(), Error> {
        for (heading, published) in [("published", true), ("not published", false)] {
            let rows: Vec<&Page<'_>> = pages.iter().filter(|page| page.decision.value == published).collect();
            io.line(format_args!("{heading} ({})", rows.len()))?;
            for page in rows {
                io.line(Row(Standing { decision: page.decision, config }))?;
            }
        }
        Ok(())
    }

    /// Every edit, planned and checked, before any is written.
    fn plan(&self, loaded: &Loaded) -> Result<Vec<Step>, Error> {
        let verb = if self.remove { Verb::Remove } else { Verb::Select };
        if let Some(written) = &self.folder {
            return Ok(vec![self.folder_step(loaded, written, verb)?]);
        }
        self.paths.iter().map(|written| self.file_step(loaded, written, verb)).collect()
    }

    fn folder_step(&self, loaded: &Loaded, written: &str, verb: Verb) -> Result<Step, Error> {
        let pattern: Pattern =
            written.parse().map_err(|source| Error::Folder { written: written.to_owned(), source })?;
        let listing = if verb == Verb::Select { Listing::Add } else { Listing::Remove };
        let existing = Self::existing(&self.dir.join(Self::CONFIG))?;
        let included = config::include_into(&existing, &pattern, listing).map_err(|source| Error::Include { source })?;
        let matches = loaded.snapshot.tree().iter().filter(|entry| pattern.matches(&entry.path)).count();
        let changed = matches!(included.listed, Listed::Added | Listed::Removed);
        Ok(Step {
            place: Place::Folder { pattern: pattern.to_string(), matches },
            outcome: included.listed.into(),
            text: changed.then_some(included.text),
        })
    }

    fn file_step(&self, loaded: &Loaded, written: &str, verb: Verb) -> Result<Step, Error> {
        let path: ContentPath =
            written.parse().map_err(|source| Error::Path { written: written.to_owned(), source })?;
        if loaded.snapshot.entry(&path).is_none() {
            return Err(Error::Unknown { path });
        }
        let pages = Pages(loaded);
        let why = if pages.ignores(&path) {
            Some(Unselectable::Ignored)
        } else if !pages.holds(&path) {
            Some(Unselectable::Plain)
        } else if !format_markdown::handles(&path) {
            Some(Unselectable::Format)
        } else {
            None
        };
        if let Some(why) = why {
            return Err(Error::Unselectable { path, why });
        }
        let place = Place::File { path: path.clone() };
        if loaded.config.pin::<Publish>(&File(path.clone())).is_some() {
            return Ok(Step { place, outcome: Outcome::Pinned, text: None });
        }
        let disk = self.dir.join(path.as_str());
        let bytes = std::fs::read(&disk).map_err(|source| Error::Read { path: disk, source })?;
        let publishing = Publishing::of(&bytes).map_err(|source| Error::Front { path: path.clone(), source })?;
        let edit = match verb {
            Verb::Select => publishing.select(),
            Verb::Remove => publishing.clear(),
        };
        let Edit { was, text, .. } = edit.map_err(|source| Error::Front { path, source })?;
        let outcome = match (verb, was, text.is_some()) {
            (Verb::Select, Some(_), true) => Outcome::Replaced,
            (Verb::Select, None, _) => Outcome::Added,
            (Verb::Select, Some(_), false) => Outcome::Already,
            (Verb::Remove, Some(true), _) => Outcome::Removed,
            (Verb::Remove, Some(false), _) => Outcome::Kept,
            (Verb::Remove, None, _) => Outcome::Absent,
        };
        Ok(Step { place, outcome, text })
    }

    /// The name of the config file in the folder.
    const CONFIG: &'static str = "ink.toml";

    /// `ink.toml`'s text, empty when there is none.
    fn existing(path: &Path) -> Result<String, Error> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(text),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(source) => Err(Error::Read { path: path.to_path_buf(), source }),
        }
    }

    /// Writes every step that has a text, unless this is a dry run.
    fn write(&self, steps: &[Step]) -> Result<Ended, Error> {
        let changed: Vec<(&Step, PathBuf)> = steps
            .iter()
            .filter(|step| step.text.is_some())
            .map(|step| {
                let at = match &step.place {
                    Place::File { path } => self.dir.join(path.as_str()),
                    Place::Folder { .. } => self.dir.join(Self::CONFIG),
                };
                (step, at)
            })
            .collect();
        if self.dry_run && !steps.is_empty() {
            return Ok(Ended::Dry);
        }
        for (step, at) in &changed {
            if let Some(text) = &step.text {
                std::fs::write(at, text).map_err(|source| Error::Write { path: at.clone(), source })?;
            }
        }
        Ok(if changed.is_empty() { Ended::Nothing } else { Ended::Wrote(changed.len()) })
    }
}
