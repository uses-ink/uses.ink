//! `ink explain [path]`: what the source had and lacked, then the decisions behind the site
//! with their evidence (a file's alone, with a path); a path that is no file of the folder is a
//! usage error. `ink explain --why [path] <code>`: one decision's ladder replayed, rung by rung
//! (decision 0042).

pub(crate) mod asked;

use std::fmt;

use engine::{Rung, Standing, Trace};
use model::{Ability, AnyDecision, ContentPath, Evidence, Gap, Lack, Observed, Subject, Version};
use serde::Serialize;

use diag::{Because, PinCommand, Short, Value};

use crate::command::explain::asked::Asked;
use crate::command::source::Source;
use crate::grammar::args::Explain;
use crate::output::error::{AskError, Error};
use crate::output::exit::Exit;
use crate::output::report::Texts;
use crate::output::term::{Cut, Doc, Flex, Glyph, Line, Rail, Render, Role, Tree, Trees};
use crate::output::{Io, Strict};

/// What `ink explain --json` prints: the version read and what the source had and lacked, then
/// each decision as storage holds it, tagged by its code.
#[derive(Debug, Serialize)]
struct Explained<'a> {
    schema: &'static str,
    version: &'a Version,
    observed: &'a Observed,
    decisions: Vec<&'a AnyDecision>,
}

/// What `ink explain --why --json` prints.
#[derive(Debug, Serialize)]
struct Traced<'a> {
    schema: &'static str,
    trace: &'a Trace,
}

impl Explain {
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
        if let Some(words) = &self.why {
            return self.why(io, Asked::words(words)?);
        }
        let subject = self.subject()?;
        let loaded = Source::new(&self.dir, &self.origin).load()?;
        if let Some(Subject::File(path)) = &subject
            && loaded.snapshot.entry(path).is_none()
        {
            return Err(AskError::Unknown { path: path.clone() }.into());
        }
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let understood = &loaded.understood;
        let tally = io.said(&understood.diagnostics, &understood.notices, texts)?;
        let decisions: Vec<&AnyDecision> = understood.explain(subject.as_ref()).collect();
        let (version, observed) = (loaded.snapshot.version(), loaded.snapshot.observed());
        let mut trees = Vec::new();
        if subject.is_none() {
            trees.push(Self::source(version, observed));
        }
        trees.extend(Self::decided(&decisions));
        io.result(&Trees(trees))?;
        io.json(&Explained { schema: "uses.explain/v2", version, observed, decisions })?;
        Ok(tally.exit(Strict::Off))
    }

    /// `--why`: interprets with `asked`'s ladder traced, and shows it.
    fn why(&self, io: &mut Io<'_>, asked: Asked) -> Result<Exit, Error> {
        let mut source = Source::new(&self.dir, &self.origin);
        source.asked = Some(asked.clone());
        let loaded = source.load()?;
        asked.within(&loaded.snapshot)?;
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let understood = &loaded.understood;
        let tally = io.said(&understood.diagnostics, &understood.notices, texts)?;
        let Some(trace) = &understood.trace else {
            return Err(AskError::Undecided { code: asked.code, subject: asked.subject }.into());
        };
        io.result(&Ladder { trace, asked: &asked })?;
        io.json(&Traced { schema: "uses.trace/v1", trace })?;
        Ok(tally.exit(Strict::Off))
    }
}

/// One decision's ladder: a rail down the rungs in order of authority, a mark for what each did,
/// and the command that writes the value down.
#[derive(Clone, Copy, Debug)]
struct Ladder<'a> {
    trace: &'a Trace,
    asked: &'a Asked,
}

impl Ladder<'_> {
    /// The sentence at the top of the rail: what was chosen, from what, and where on the ladder.
    fn sentence(&self) -> Line {
        let text = match self.trace.chosen() {
            Some((at, heard)) => format!(
                "{}, {}: rung {at} of {}",
                Value(&heard.testimony),
                Because(heard.testimony.evidence()),
                self.trace.rungs.len()
            ),
            None => "no rung said a value".to_owned(),
        };
        Line::plain(text)
    }

    /// The rows of one rung: its mark and place, its witness, and what each testimony said.
    fn rows(at: usize, rung: &Rung, into: &Doc) -> Vec<Vec<Line>> {
        let witness = Line::plain(rung.witness.to_string());
        let chosen = rung.heard.iter().any(|heard| heard.chosen);
        let mark = |glyph| into.glyph(glyph).pad(1).with(Role::Plain, at.to_string());
        if rung.heard.is_empty() {
            return vec![vec![mark(Glyph::Gone), witness, Line::new().with(Role::Quiet, "nothing")]];
        }
        rung.heard
            .iter()
            .enumerate()
            .map(|(row, heard)| {
                let (verdict, role) = match (heard.chosen, heard.standing) {
                    (true, _) => ("chosen", Role::Good),
                    (false, Standing::Bound) => ("bound", Role::Quiet),
                    (false, _) => ("outranked", Role::Quiet),
                };
                let (first, second) = if row == 0 {
                    (mark(if chosen { Glyph::Done } else { Glyph::Outranked }), witness.clone())
                } else {
                    (Line::new(), Line::new())
                };
                let evidence = Because(heard.testimony.evidence()).to_string();
                vec![
                    first,
                    second,
                    Line::plain(Value(&heard.testimony).to_string()),
                    Line::new().with(role, verdict),
                    Line::new().with(Role::Quiet, evidence),
                ]
            })
            .collect()
    }

    /// The last line: how to pin the value.
    fn foot(&self) -> Line {
        let command = PinCommand(&self.asked.code, &self.asked.subject).to_string();
        Line::new().with(Role::Quiet, "to pin it: ").with(Role::Literal, command)
    }
}

impl Render for Ladder<'_> {
    fn render(&self, into: &mut Doc) {
        let trace = self.trace;
        into.push(Line::new().with(Role::Heading, format!("{} on {}", trace.code, trace.subject)));
        let rows = trace.rungs.iter().enumerate().flat_map(|(at, rung)| Self::rows(at + 1, rung, into)).collect();
        Rail { top: self.sentence(), rows, foot: self.foot(), flex: Some(Flex { column: 4, cut: Cut::End }) }
            .render(into);
    }
}

impl Explain {
    /// The file asked about, or `None` for the whole site.
    fn subject(&self) -> Result<Option<Subject>, Error> {
        let Some(written) = &self.path else { return Ok(None) };
        let path: ContentPath =
            written.parse().map_err(|source| AskError::Path { written: written.clone(), source })?;
        Ok(Some(Subject::File(path)))
    }

    /// What the source was read at, what it had and what it lacked.
    fn source(version: &Version, observed: &Observed) -> Tree {
        let key = |key: &str, value: String| vec![Line::new().with(Role::Quiet, key), Line::plain(value)];
        let mut tree = Tree::new(Line::new().with(Role::Heading, "the source"))
            .row(key("version", Read(version).to_string()))
            .row(key("has", Has(&observed.has).to_string()));
        for lack in &observed.lacks {
            tree = tree.row(key("lacks", Lacking(lack).to_string()));
        }
        tree.flex(1, Cut::End)
    }

    /// One tree for each subject, in the order the decisions come: a row for each decision with
    /// its code, its value in that code's words and its evidence, which is dim unless the site's
    /// own pin chose the value.
    fn decided(decisions: &[&AnyDecision]) -> Vec<Tree> {
        let mut trees: Vec<Tree> = Vec::new();
        let mut current: Option<Subject> = None;
        for decision in decisions {
            let about = decision.subject();
            if current.as_ref() != Some(&about) {
                trees.push(Tree::new(Line::new().with(Role::Heading, about.to_string())).flex(2, Cut::End));
                current = Some(about);
            }
            let role = if matches!(decision.evidence(), Evidence::Config { .. }) { Role::Id } else { Role::Quiet };
            let row = vec![
                Line::new().with(Role::Code, decision.code().name()),
                Line::plain(Value(decision).to_string()),
                Line::new().with(role, Because(decision.evidence()).to_string()),
            ];
            if let Some(tree) = trees.pop() {
                trees.push(tree.row(row));
            }
        }
        trees
    }
}

/// The version a snapshot was read at.
#[derive(Clone, Copy, Debug)]
struct Read<'a>(&'a Version);

impl fmt::Display for Read<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Version::GitCommit(commit) => write!(out, "git commit {commit}"),
            // its hash is in `--json`; here it would change with every edit
            Version::Folder { .. } => write!(out, "the files on disk"),
            Version::Upload(id) => write!(out, "upload {id}"),
            _ => write!(out, "a version this ink cannot show"),
        }
    }
}

/// What a source could do, in order.
#[derive(Clone, Copy, Debug)]
struct Has<'a>(&'a [Ability]);

impl fmt::Display for Has<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (at, ability) in self.0.iter().enumerate() {
            if at > 0 {
                out.write_str(", ")?;
            }
            out.write_str(match ability {
                Ability::Snapshot => "snapshot",
                Ability::History => "history",
                Ability::Watch => "watch",
                Ability::Private => "private",
                _ => "an ability this ink cannot show",
            })?;
        }
        Ok(())
    }
}

/// What a source lacked, and the decision that suffers for it.
#[derive(Clone, Copy, Debug)]
struct Lacking<'a>(&'a Lack);

impl fmt::Display for Lacking<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let decision = self.0.consequence.name();
        match &self.0.gap {
            Gap::Absent => write!(out, "{}, so {decision} is not taken from it", Has(&[self.0.ability])),
            Gap::Shallow { commit, bound } => write!(
                out,
                "history before commit {} (shallow), so {decision} of a file not changed since is a bound, not after {bound}",
                Short(commit)
            ),
            Gap::Renames { moved } => {
                write!(out, "renames, so {decision} of a file that moved ({moved} here) is when it moved")
            }
            _ => write!(out, "{} in part, as this ink cannot show", Has(&[self.0.ability])),
        }
    }
}
