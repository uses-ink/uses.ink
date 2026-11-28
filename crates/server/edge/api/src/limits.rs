//! What the API and intake spend (decision 0044, bucket 2: the operator's ceilings).

use std::fmt;

use crate::archive;

/// The budgets of one call. Every field is the operator's to set (a flag of `ink serve`), and a
/// site's own configuration may only lower it: see [`Limits::lower`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// A JSON request body, in bytes.
    pub body: u64,
    /// An uploaded archive, in bytes.
    pub archive: u64,
    /// Files in an uploaded archive.
    pub files: u64,
    /// Bytes in one uploaded file.
    pub file: u64,
    /// Bytes in a name inside an archive.
    pub name: usize,
    /// An exported archive, in bytes.
    pub export: u64,
    /// Rows in a list.
    pub rows: u32,
    /// A tree sent by `ink push`, in bytes of JSON.
    pub tree: u64,
    /// The files one upload request carries together, in bytes: the most memory a request of a
    /// push holds.
    pub part: u64,
    /// A forge's webhook body, in bytes. It is the operator's: a site's configuration does not
    /// reach it, and it is set with the forge authority's own limit of the same name.
    pub hook: u64,
}

impl Default for Limits {
    fn default() -> Self {
        const MIB: u64 = 1024 * 1024;
        Self {
            body: 64 * 1024,
            archive: 256 * MIB,
            files: 10_000,
            file: 64 * MIB,
            name: 1024,
            export: 256 * MIB,
            rows: 50,
            tree: 8 * MIB,
            part: 64 * MIB,
            hook: 5 * MIB,
        }
    }
}

/// A limit, by the name an operator or a site's configuration knows it by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Limit {
    /// [`Limits::body`].
    Body,
    /// [`Limits::archive`].
    Archive,
    /// [`Limits::files`].
    Files,
    /// [`Limits::file`].
    File,
    /// [`Limits::name`].
    Name,
    /// [`Limits::export`].
    Export,
    /// [`Limits::rows`].
    Rows,
    /// [`Limits::tree`].
    Tree,
    /// [`Limits::part`].
    Part,
}

impl fmt::Display for Limit {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Body => "body",
            Self::Archive => "archive",
            Self::Files => "files",
            Self::File => "file",
            Self::Name => "name",
            Self::Export => "export",
            Self::Rows => "rows",
            Self::Tree => "tree",
            Self::Part => "part",
        })
    }
}

/// A value for a limit that is above the operator's ceiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Raised {
    /// Which limit.
    pub limit: Limit,
    /// The operator's value.
    pub ceiling: u64,
    /// What was asked for.
    pub asked: u64,
}

impl fmt::Display for Raised {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "{} is {}, above the operator's limit of {}: a setting may lower a limit and never raise it",
            self.limit, self.asked, self.ceiling
        )
    }
}

impl std::error::Error for Raised {}

/// The values a site asks for, each one only when it asks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Asked {
    /// [`Limits::body`].
    pub body: Option<u64>,
    /// [`Limits::archive`].
    pub archive: Option<u64>,
    /// [`Limits::files`].
    pub files: Option<u64>,
    /// [`Limits::file`].
    pub file: Option<u64>,
    /// [`Limits::export`].
    pub export: Option<u64>,
    /// [`Limits::rows`].
    pub rows: Option<u32>,
    /// [`Limits::tree`].
    pub tree: Option<u64>,
    /// [`Limits::part`].
    pub part: Option<u64>,
}

impl Limit {
    /// `asked` under `ceiling`: the lower of the two, or [`Raised`] when it is above.
    fn under(self, ceiling: u64, asked: Option<u64>) -> Result<u64, Raised> {
        match asked {
            Some(asked) if asked > ceiling => Err(Raised { limit: self, ceiling, asked }),
            Some(asked) => Ok(asked),
            None => Ok(ceiling),
        }
    }
}

impl Limits {
    /// These limits lowered to what `asked` says. A value above the limit is [`Raised`], which
    /// names both numbers; nothing is clamped silently.
    pub fn lower(self, asked: &Asked) -> Result<Self, Raised> {
        let rows = Limit::Rows.under(u64::from(self.rows), asked.rows.map(u64::from))?;
        Ok(Self {
            body: Limit::Body.under(self.body, asked.body)?,
            archive: Limit::Archive.under(self.archive, asked.archive)?,
            files: Limit::Files.under(self.files, asked.files)?,
            file: Limit::File.under(self.file, asked.file)?,
            name: self.name,
            export: Limit::Export.under(self.export, asked.export)?,
            rows: u32::try_from(rows).unwrap_or(self.rows),
            tree: Limit::Tree.under(self.tree, asked.tree)?,
            part: Limit::Part.under(self.part, asked.part)?,
            hook: self.hook,
        })
    }

    /// How many body bytes `method` of `path` may send: an archive for intake, a small JSON
    /// document for the rest. An adapter reads no more than this.
    pub fn budget(&self, method: crate::Method, path: &str) -> u64 {
        if path.starts_with(crate::HOOKS) {
            return self.hook;
        }
        match crate::Call::parse(method, path) {
            Ok(crate::Call::Intake(_)) => self.archive,
            Ok(crate::Call::Source(_)) => self.tree,
            Ok(crate::Call::Parts(_)) => self.part,
            _ => self.body,
        }
    }

    /// What an archive may hold under these limits.
    pub const fn archives(&self) -> archive::Limits {
        archive::Limits { files: self.files, file: self.file, total: self.archive, name: self.name }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_setting_lowers_a_limit_and_never_raises_it() {
        let ceiling = Limits::default();
        let lowered = ceiling.lower(&Asked { files: Some(10), ..Asked::default() }).unwrap();
        assert_eq!((lowered.files, lowered.archive), (10, ceiling.archive));
        let raised = ceiling.lower(&Asked { files: Some(ceiling.files + 1), ..Asked::default() }).unwrap_err();
        assert_eq!(raised, Raised { limit: Limit::Files, ceiling: ceiling.files, asked: ceiling.files + 1 });
        assert!(raised.to_string().contains("10001"));
    }
}
