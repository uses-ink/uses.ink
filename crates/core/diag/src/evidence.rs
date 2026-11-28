//! A decision's value and where it came from, in words: what `ink explain` and the dashboard's
//! "What we understood" screen both say.

use std::fmt;

use model::{AnyCode, AnyDecision, Evidence, Oid, Provenance, Subject};

/// A decision's value, as its code words it.
#[derive(Clone, Copy, Debug)]
pub struct Value<'a>(pub &'a AnyDecision);

impl fmt::Display for Value<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.word(out)
    }
}

/// Where a decision came from.
#[derive(Clone, Copy, Debug)]
pub struct Because<'a>(pub &'a Evidence);

impl fmt::Display for Because<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Evidence::Frontmatter { path, span } => write!(out, "from the frontmatter of {path}, {span}"),
            Evidence::Filename { path } => write!(out, "from the name of {path}"),
            Evidence::Heading { path, span } => write!(out, "from the heading of {path}, {span}"),
            Evidence::Inline { path, span } => write!(out, "from the inline text of {path}, {span}"),
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
pub struct Short<'a>(pub &'a Oid);

impl fmt::Display for Short<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.0.as_str().get(..12).unwrap_or(self.0.as_str()))
    }
}

/// The command that pins a decision: `ink pin posts/x.md page.title`, or `ink pin site.home` for
/// the site's own.
#[derive(Clone, Copy, Debug)]
pub struct PinCommand<'a>(pub &'a AnyCode, pub &'a Subject);

impl fmt::Display for PinCommand<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.1 {
            Subject::File(path) => write!(out, "ink pin {path} {}", self.0),
            _ => write!(out, "ink pin {}", self.0),
        }
    }
}
