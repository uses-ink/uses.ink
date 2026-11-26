//! `ink explain [--pin] [path]`: what the source had and lacked, then the decisions behind the
//! site with their evidence (a file's alone, with a path), or every decision as `ink.toml` pins. With a path, only that file's
//! decisions, shown or pinned; a path that is no file of the folder is a usage error.

use std::fmt;

use model::{Ability, AnyDecision, ContentPath, Evidence, Gap, Lack, Observed, Oid, Provenance, Subject, Version};
use serde::Serialize;

use crate::args::Explain;
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

/// What `ink explain --pin --json` prints.
#[derive(Debug, Serialize)]
struct Pinned {
    schema: &'static str,
    toml: String,
}

impl Explain {
    pub(crate) fn run(self, io: &mut Io<'_>) -> Result<Exit, Error> {
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
        if self.pin {
            let toml = config::pins(understood.explain(subject.as_ref()));
            if io.is_json() {
                io.json(&Pinned { schema: "uses.pin/v1", toml })?;
            } else {
                io.raw(&toml)?;
            }
            return Ok(tally.exit(Strict::Off));
        }
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
        io.json(&Explained { schema: "uses.explain/v1", version, observed, decisions })?;
        Ok(tally.exit(Strict::Off))
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
struct Value<'a>(&'a AnyDecision);

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
