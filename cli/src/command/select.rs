//! `ink select`: what is published and why, and recording a selection (decisions 0042, open.md
//! Q16 and Q17). In a vault or a private source a page is private until it is selected, by
//! `publish: true` in its frontmatter or an `include` pattern in `ink.toml`; in a public source
//! every page is published unless it says otherwise, and selecting only states it.
//!
//! The listing is the decision `page.publish` and its evidence. A selection is planned whole
//! before anything is written: one refused file stops all of them. It reads the files on disk,
//! since it writes into them, and writes them together (`Replacing`): every text goes to a
//! temporary file first, so a failed write leaves every file as it was.

use std::fmt;
use std::path::{Path, PathBuf};

use config::{Config, Listed, Listing, Pattern};
use diag::Because;
use engine::Selection;
use format_markdown::{Edit, Publishing};
use model::code::{Ignored, Kind, Publish};
use model::decision::File;
use model::{AnyDecision, ContentPath, Decision, DefaultReason, Evidence};
use serde::Serialize;

use crate::command::source::{Loaded, Source};
use crate::folder::replace::Replacing;
use crate::grammar::args::{Kind as Reading, Origin, Select};
use crate::output::error::Error;
use crate::output::exit::Exit;
use crate::output::report::Texts;
use crate::output::stage::{Plural, StageLine, Verb as Stage};
use crate::output::term::{Cut, Doc, Glyph, Line, Render, Role, Table};
use crate::output::{Io, Strict};

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
pub(crate) enum Outcome {
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

impl Outcome {
    /// The mark of what came of the step: added, removed, or nothing.
    const fn glyph(self) -> Glyph {
        match self {
            Self::Added | Self::Replaced | Self::Listed => Glyph::Added,
            Self::Removed | Self::Delisted => Glyph::Removed,
            _ => Glyph::Gone,
        }
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

impl Step {
    /// The row of this step: its mark, its place, and what came of it.
    fn row(&self, into: &Doc) -> Vec<Line> {
        let said = match &self.place {
            Place::Folder { matches, .. } => format!("{}, matches {}", self.outcome, Plural(*matches, "file")),
            Place::File { .. } => self.outcome.to_string(),
        };
        vec![into.glyph(self.outcome.glyph()), Line::plain(self.place.to_string()), Line::new().with(Role::Quiet, said)]
    }
}

/// How a selection ended.
#[derive(Clone, Copy, Debug)]
enum Ended {
    Wrote(usize),
    Dry,
    Nothing,
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
            Selection::Explicit => {
                out.write_str("selection: explicit, a vault or private source: a page is private until it is selected")
            }
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

/// What `ink select` lists: how pages are selected, then the pages that are published and those
/// that are private, each with why.
#[derive(Clone, Copy, Debug)]
struct Overview<'a> {
    selecting: Selecting,
    pages: &'a [Page<'a>],
    config: &'a Config,
}

impl Render for Overview<'_> {
    fn render(&self, into: &mut Doc) {
        into.push(Line::new().with(Role::Quiet, self.selecting.to_string()));
        for (heading, published) in [("published", true), ("private", false)] {
            let rows: Vec<&Page<'_>> = self.pages.iter().filter(|page| page.decision.value == published).collect();
            let title = if published {
                format!("{heading} ({} of {})", rows.len(), self.pages.len())
            } else {
                format!("{heading} ({})", rows.len())
            };
            let glyph = if published { Glyph::Live } else { Glyph::Private };
            let mut table = Table::new(Line::new().with(Role::Heading, title)).indent(2).flex(2, Cut::End);
            for page in rows {
                let why = Standing { decision: page.decision, config: self.config }.to_string();
                let path = page.decision.subject.0.to_string();
                table = table.row(vec![into.glyph(glyph), Line::plain(path), Line::new().with(Role::Quiet, why)]);
            }
            into.blank();
            table.render(into);
        }
    }
}

/// What a selection did: the stage line, then each step.
#[derive(Clone, Copy, Debug)]
struct Applied<'a> {
    steps: &'a [Step],
    ended: Ended,
    removing: bool,
}

impl Render for Applied<'_> {
    fn render(&self, into: &mut Doc) {
        let changed = self.steps.iter().filter(|step| step.text.is_some()).count();
        let files = Plural(changed, "file");
        let line = match (self.ended, self.removing) {
            (Ended::Wrote(count), false) => StageLine::new(Stage::Selected, Plural(count, "file")),
            (Ended::Wrote(count), true) => StageLine::new(Stage::Deselected, Plural(count, "file")),
            (Ended::Dry, false) => StageLine::new(Stage::WouldSelect, files),
            (Ended::Dry, true) => StageLine::new(Stage::WouldDeselect, files),
            (Ended::Nothing, _) => StageLine::new(Stage::Unchanged, "nothing to write"),
        };
        line.render(into);
        let mut table = Table::default().indent(2).flex(2, Cut::End);
        for step in self.steps {
            table = table.row(step.row(into));
        }
        table.render(into);
        if matches!(self.ended, Ended::Wrote(_)) && !self.removing {
            into.blank();
            let note = "Nothing is public until you build and push: `ink build` shows it as newly public.";
            into.push(Line::marked(Role::Plain, note));
        }
    }
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
            io.result(&Overview { selecting: Selecting(understood.selection), pages: &pages, config: &loaded.config })?;
        } else {
            io.result(&Applied { steps: &steps, ended, removing: self.remove })?;
        }
        let written = if let Ended::Wrote(count) = ended { count } else { 0 };
        io.json(&Selected {
            schema: "uses.select/v1",
            selection: understood.selection,
            pages,
            steps: &steps,
            written,
        })?;
        Ok(tally.exit(Strict::Off))
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
        let included =
            config::include_into(&existing, &pattern, listing).map_err(|source| Error::Include { source })?;
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
        let publishing =
            Publishing::of(&bytes).map_err(|source| Error::Front { path: path.clone(), source: Box::new(source) })?;
        let edit = match verb {
            Verb::Select => publishing.select(),
            Verb::Remove => publishing.clear(),
        };
        let Edit { was, text, .. } = edit.map_err(|source| Error::Front { path, source: Box::new(source) })?;
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
        let mut replacing = Replacing::default();
        for (step, at) in changed {
            replacing.push(at, step.text.clone().unwrap_or_default());
        }
        Ok(match replacing.commit()? {
            0 => Ended::Nothing,
            count => Ended::Wrote(count),
        })
    }
}
