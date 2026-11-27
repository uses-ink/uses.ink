//! A build's change set as a person reads it (decision 0042): what it compares with, newly
//! public files first, then what is no longer served, what moved, and each changed decision
//! with its cause.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use model::Subject;
use model::record::{Against, Change, Changes};

use diag::Value;

/// The change set of a build into the output folder `out`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Listed<'a> {
    pub(crate) changes: &'a Changes,
    pub(crate) out: &'a Path,
}

impl fmt::Display for Listed<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let changes = self.changes;
        let folder = self.out.display();
        match &changes.against {
            Against::Deploy { deploy } => writeln!(out, "against the previous build in {folder}, deploy {deploy}")?,
            Against::Unrecorded { .. } => {
                writeln!(out, "against the previous build in {folder}, which kept no decision record to compare")?;
            }
            Against::Nothing => writeln!(out, "against nothing: the first build in {folder}")?,
            _ => writeln!(out, "against a baseline this ink cannot show")?,
        }
        if changes.is_empty() {
            return writeln!(out, "no change");
        }
        write!(out, "{}", Routes(changes))?;
        if !changes.decisions.is_empty() {
            writeln!(out, "changed")?;
            for group in Group::all(&changes.decisions) {
                write!(out, "{group}")?;
            }
        }
        writeln!(
            out,
            "{} newly public, {} no longer public, {} moved, {} decisions changed",
            changes.public.len(),
            changes.withdrawn.len(),
            changes.moved.len(),
            changes.decisions.len()
        )
    }
}

/// What a change set says about routes: newly public, no longer public, moved.
#[derive(Clone, Copy, Debug)]
struct Routes<'a>(&'a Changes);

impl fmt::Display for Routes<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let changes = self.0;
        if !changes.public.is_empty() {
            writeln!(out, "newly public")?;
            for routed in &changes.public {
                writeln!(out, "  {:<32} {}", routed.path.as_str(), routed.route)?;
            }
        }
        if !changes.withdrawn.is_empty() {
            writeln!(out, "no longer public")?;
            for routed in &changes.withdrawn {
                writeln!(out, "  {:<32} {}  the address is no longer served", routed.path.as_str(), routed.route)?;
            }
        }
        if !changes.moved.is_empty() {
            writeln!(out, "moved")?;
            for moved in &changes.moved {
                let path = moved.path.as_str();
                writeln!(out, "  {path:<32} {} -> {}  the old address is no longer served", moved.before, moved.after)?;
            }
        }
        Ok(())
    }
}

/// The changes of one subject: one line when all are new or all are gone, else one per change.
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
}

impl fmt::Display for Group<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Some(first) = self.0.first() else { return Ok(()) };
        let subject = Named(&first.subject).to_string();
        if self.0.iter().all(|change| change.before.is_none()) {
            return writeln!(out, "  {subject:<32} new, {}", first.cause);
        }
        if self.0.iter().all(|change| change.after.is_none()) {
            return writeln!(out, "  {subject:<32} gone, {}", first.cause);
        }
        for change in &self.0 {
            let was = Shown(change.before.as_ref()).to_string();
            let is = Shown(change.after.as_ref()).to_string();
            writeln!(out, "  {subject:<32} {} {was} -> {is}  {}", change.code, change.cause)?;
        }
        Ok(())
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
