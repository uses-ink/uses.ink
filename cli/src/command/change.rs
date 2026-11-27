//! A build's change set as a block (decisions 0042 and 0061): what it compares with, newly public
//! files first, then what is no longer served, what moved, and each changed decision with its
//! cause. Its JSON form is `changes` in `uses.build/v2`.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use model::Subject;
use model::record::{Against, Cause, Change, Changes};

use crate::stage::{Plural, StageLine, Verb};
use crate::term::{Cut, Doc, Glyph, Line, Render, Role, Table, Tree};
use diag::Value;

/// How much of a change set is written: all of it, or only what a build made public, which is the
/// one thing in a build that cannot be undone (decision 0042).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    Whole,
    Public,
}

/// The change set of a build into the output folder `out`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Listed<'a> {
    pub(crate) changes: &'a Changes,
    pub(crate) out: &'a Path,
    pub(crate) scope: Scope,
}

/// What a change set compares with, as the heading says it.
#[derive(Clone, Copy, Debug)]
struct Baseline<'a>(&'a Against, &'a Path);

impl fmt::Display for Baseline<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Against::Deploy { deploy } => write!(out, "against deploy {deploy}"),
            Against::Unrecorded { .. } => {
                write!(
                    out,
                    "against the previous build in {}, which kept no decision record to compare",
                    self.1.display()
                )
            }
            Against::Nothing => write!(out, "the first build in {}", self.1.display()),
            _ => write!(out, "against a baseline this ink cannot show"),
        }
    }
}

impl Listed<'_> {
    /// The heading: the counts, and what they are against.
    fn heading(&self) -> StageLine {
        let changes = self.changes;
        let baseline = Baseline(&changes.against, self.out);
        if changes.is_empty() {
            return StageLine::new(Verb::Changes, format!("no change, {baseline}"));
        }
        let counts = [
            format!("{} newly public", changes.public.len()),
            format!("{} no longer public", changes.withdrawn.len()),
            format!("{} moved", changes.moved.len()),
            Plural(changes.decisions.len(), "decision").to_string() + " changed",
        ];
        StageLine::new(Verb::Changes, format!("{}, {baseline}", counts.join(", ")))
    }

    /// A section's title: `newly public (2)`.
    fn title(words: &str, count: usize) -> Line {
        Line::new().with(Role::Heading, format!("{words} ({count})"))
    }

    /// The rows of newly public, no longer public and moved.
    fn routes(&self, into: &Doc) -> Vec<Table> {
        let changes = self.changes;
        let mut tables = Vec::new();
        let row = |glyph: Glyph, path: &str, route: String, note: &str| {
            vec![into.glyph(glyph), Line::plain(path), Line::plain(route), Line::new().with(Role::Quiet, note)]
        };
        if !changes.public.is_empty() {
            let mut table =
                Table::new(Self::title("newly public", changes.public.len())).indent(2).flex(1, Cut::Middle);
            for routed in &changes.public {
                table = table.row(row(Glyph::Added, routed.path.as_str(), routed.route.to_string(), ""));
            }
            tables.push(table);
        }
        if self.scope == Scope::Public {
            return tables;
        }
        if !changes.withdrawn.is_empty() {
            let mut table =
                Table::new(Self::title("no longer public", changes.withdrawn.len())).indent(2).flex(1, Cut::Middle);
            for routed in &changes.withdrawn {
                let note = "the address is no longer served";
                table = table.row(row(Glyph::Removed, routed.path.as_str(), routed.route.to_string(), note));
            }
            tables.push(table);
        }
        if !changes.moved.is_empty() {
            let mut table = Table::new(Self::title("moved", changes.moved.len())).indent(2).flex(1, Cut::Middle);
            for moved in &changes.moved {
                let arrow = into.look().glyphs.get(Glyph::To);
                let route = format!("{} {arrow} {}", moved.before, moved.after);
                table =
                    table.row(row(Glyph::Changed, moved.path.as_str(), route, "the old address is no longer served"));
            }
            tables.push(table);
        }
        tables
    }
}

impl Render for Listed<'_> {
    fn render(&self, into: &mut Doc) {
        let changes = self.changes;
        if self.scope == Scope::Whole {
            self.heading().render(into);
        }
        if changes.is_empty() {
            return;
        }
        for table in self.routes(into) {
            if self.scope == Scope::Whole {
                into.blank();
            }
            table.render(into);
        }
        if self.scope == Scope::Whole && !changes.decisions.is_empty() {
            into.blank();
            into.push(Self::title("decisions changed", changes.decisions.len()));
            for group in Group::all(&changes.decisions) {
                group.tree(into).render(into);
            }
        }
        if self.scope == Scope::Whole || !changes.public.is_empty() {
            into.blank();
        }
    }
}

/// The changes of one subject: one row when all are new or all are gone, else one per change.
#[derive(Clone, Debug)]
struct Group<'a>(Vec<&'a Change>);

impl<'a> Group<'a> {
    /// `changes` grouped by subject, each subject where it first appears.
    fn all(changes: &'a [Change]) -> Vec<Self> {
        let mut groups: Vec<Self> = Vec::new();
        let mut at: BTreeMap<&Subject, usize> = BTreeMap::new();
        for change in changes {
            let index = *at.entry(&change.subject).or_insert_with(|| {
                groups.push(Self(Vec::new()));
                groups.len() - 1
            });
            if let Some(group) = groups.get_mut(index) {
                group.0.push(change);
            }
        }
        groups
    }

    /// The group as a tree under its subject.
    fn tree(&self, into: &Doc) -> Tree {
        let Some(first) = self.0.first() else { return Tree::default() };
        let subject = Named(&first.subject).to_string();
        let tree = Tree::new(Line::new().with(Role::Heading, subject)).flex(1, Cut::End);
        let cause = |change: &Change| {
            let role = if change.cause == Cause::Engine { Role::Warn } else { Role::Quiet };
            Line::new().with(role, change.cause.to_string())
        };
        if self.0.iter().all(|change| change.before.is_none()) {
            return tree.row(vec![Line::new().with(Role::Good, "new"), cause(first)]);
        }
        if self.0.iter().all(|change| change.after.is_none()) {
            return tree.row(vec![Line::new().with(Role::Bad, "gone"), cause(first)]);
        }
        let arrow = into.look().glyphs.get(Glyph::To);
        self.0.iter().fold(tree, |tree, change| {
            let was = Shown(change.before.as_ref()).to_string();
            let is = Shown(change.after.as_ref()).to_string();
            tree.row(vec![
                Line::new().with(Role::Code, change.code.to_string()),
                Line::plain(format!("{was} {arrow} {is}")),
                cause(change),
            ])
        })
    }
}

/// A subject as the change set names it: a file by its path.
#[derive(Clone, Copy, Debug)]
struct Named<'a>(&'a Subject);

impl fmt::Display for Named<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Subject::File(path) => out.write_str(path.as_str()),
            other => write!(out, "{other}"),
        }
    }
}

/// A decision's value in its code's words, or that it was not made.
#[derive(Clone, Copy, Debug)]
struct Shown<'a>(Option<&'a model::AnyDecision>);

impl fmt::Display for Shown<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(decision) => write!(out, "{}", Value(decision)),
            None => out.write_str("not made"),
        }
    }
}
