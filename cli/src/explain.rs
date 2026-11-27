//! `ink explain [path]`: what the source had and lacked, then the decisions behind the site
//! with their evidence (a file's alone, with a path); a path that is no file of the folder is a
//! usage error. `ink explain --why [path] <code>`: one decision's ladder replayed, rung by rung
//! (decision 0042).

use std::fmt;

use engine::{Heard, Rung, Standing, Trace};
use model::{Ability, AnyDecision, ContentPath, Evidence, Gap, Lack, Observed, Oid, Provenance, Subject, Version};
use serde::Serialize;

use crate::args::Explain;
use crate::asked::Asked;
use crate::error::Error;
use crate::exit::Exit;
use crate::output::{Io, Strict};
use crate::report::Texts;
use crate::site::Source;

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
            return Err(Error::Unknown { path: path.clone() });
        }
        let texts = Texts { snapshot: Some(&loaded.snapshot), store: Some(&loaded.store) };
        let understood = &loaded.understood;
        let tally = io.said(&understood.diagnostics, &understood.notices, texts)?;
        let decisions: Vec<&AnyDecision> = understood.explain(subject.as_ref()).collect();
        let (version, observed) = (loaded.snapshot.version(), loaded.snapshot.observed());
        if subject.is_none() {
            Self::source(io, version, observed)?;
        }
        let mut current: Option<Subject> = None;
        for decision in &decisions {
            let about = decision.subject();
            if current.as_ref() != Some(&about) {
                io.line(&about)?;
                current = Some(about);
            }
            io.line(Line(decision))?;
        }
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
            return Err(Error::Undecided { code: asked.code, subject: asked.subject });
        };
        io.line(Head(trace))?;
        for (at, rung) in trace.rungs.iter().enumerate() {
            io.line(Step { at: at + 1, rung })?;
        }
        io.line(Hint(&asked))?;
        io.json(&Traced { schema: "uses.trace/v1", trace })?;
        Ok(tally.exit(Strict::Off))
    }
}

/// A trace's first line: the decision, its evidence and its place on the ladder.
#[derive(Clone, Copy, Debug)]
struct Head<'a>(&'a Trace);

impl fmt::Display for Head<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{} on {}", self.0.code, self.0.subject)?;
        match self.0.chosen() {
            Some((at, heard)) => write!(
                out,
                ": {} {}, rung {at} of {}",
                Value(&heard.testimony),
                Because(heard.testimony.evidence()),
                self.0.rungs.len()
            ),
            None => out.write_str(": no rung said a value"),
        }
    }
}

/// One rung: its place, its witness, and what it said or that it found nothing.
#[derive(Clone, Copy, Debug)]
struct Step<'a> {
    at: usize,
    rung: &'a Rung,
}

impl fmt::Display for Step<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let witness = self.rung.witness.to_string();
        write!(out, "  {:>2} {witness:<18}", self.at)?;
        if self.rung.heard.is_empty() {
            return out.write_str(" nothing");
        }
        for (at, heard) in self.rung.heard.iter().enumerate() {
            if at > 0 {
                write!(out, "\n  {:>2} {:<18}", "", "")?;
            }
            write!(out, " {}", Testified(heard))?;
        }
        Ok(())
    }
}

/// One testimony: its value, its evidence, and how it stood.
#[derive(Clone, Copy, Debug)]
struct Testified<'a>(&'a Heard);

impl fmt::Display for Testified<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let heard = self.0;
        let value = Value(&heard.testimony).to_string();
        write!(out, "{value:<24} {}, ", Because(heard.testimony.evidence()))?;
        out.write_str(match (heard.chosen, heard.standing) {
            (true, _) => "chosen",
            (false, Standing::Bound) => "a bound, never chosen",
            (false, _) => "outranked",
        })
    }
}

/// How to fix the value: the command that pins it.
#[derive(Clone, Copy, Debug)]
struct Hint<'a>(&'a Asked);

impl fmt::Display for Hint<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0.subject {
            Subject::File(path) => write!(out, "to pin it: ink pin {path} {}", self.0.code),
            _ => write!(out, "to pin it: ink pin {}", self.0.code),
        }
    }
}

impl Explain {
    /// The file asked about, or `None` for the whole site.
    fn subject(&self) -> Result<Option<Subject>, Error> {
        let Some(written) = &self.path else { return Ok(None) };
        let path: ContentPath = written.parse().map_err(|source| Error::Path { written: written.clone(), source })?;
        Ok(Some(Subject::File(path)))
    }

    /// What the source was read at, what it had and what it lacked.
    fn source(io: &mut Io<'_>, version: &Version, observed: &Observed) -> Result<(), Error> {
        io.line("the source")?;
        io.line(format_args!("  {:<18} {}", "version", Read(version)))?;
        io.line(format_args!("  {:<18} {}", "has", Has(&observed.has)))?;
        for lack in &observed.lacks {
            io.line(format_args!("  {:<18} {}", "lacks", Lacking(lack)))?;
        }
        Ok(())
    }
}

/// One decision: `  code  value  from evidence`, its value in its code's words.
#[derive(Clone, Copy, Debug)]
struct Line<'a>(&'a AnyDecision);

impl fmt::Display for Line<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = Value(self.0).to_string();
        write!(out, "  {:<18} {value:<24} {}", self.0.code().name(), Because(self.0.evidence()))
    }
}

/// A decision's value, as its code words it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Value<'a>(pub(crate) &'a AnyDecision);

impl fmt::Display for Value<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.word(out)
    }
}

/// Where a decision came from.
#[derive(Clone, Copy, Debug)]
struct Because<'a>(&'a Evidence);

impl fmt::Display for Because<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Evidence::Frontmatter { path, span } => write!(out, "from the frontmatter of {path}, {span}"),
            Evidence::Filename { path } => write!(out, "from the name of {path}"),
            Evidence::Heading { path, span } => write!(out, "from the heading of {path}, {span}"),
            Evidence::History { from: Provenance::Commit { commit } } => write!(out, "from commit {}", Short(commit)),
            Evidence::History { from: Provenance::GitHistory { commit } } => {
                write!(out, "from the history up to commit {}", Short(commit))
            }
            Evidence::History { from: Provenance::Filesystem } => write!(out, "from the file's modification time"),
            Evidence::History { .. } => write!(out, "from the history"),
            Evidence::Body { path } => write!(out, "from the text of {path}"),
            Evidence::Marker { path } => write!(out, "because {path} is there"),
            Evidence::Config { span } => write!(out, "pinned in ink.toml, {span}"),
            Evidence::Default { .. } => write!(out, "by default"),
            _ => write!(out, "from evidence this version cannot show"),
        }
    }
}

/// A commit id as people write it: its first twelve digits.
#[derive(Clone, Copy, Debug)]
struct Short<'a>(&'a Oid);

impl fmt::Display for Short<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.0.as_str().get(..12).unwrap_or(self.0.as_str()))
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
