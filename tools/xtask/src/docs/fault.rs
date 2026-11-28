//! What the docs checks find wrong: one [`Fault`] each, with where it is.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use super::front::Defect;
use super::number::Number;

/// One thing a docs check found wrong.
#[derive(Debug)]
pub(crate) enum Fault {
    /// A relative link that names no path.
    Broken { file: PathBuf, line: usize, target: String },
    /// A citation of a decision that has no file and is not reserved.
    Cite { file: PathBuf, line: usize, number: Number },
    /// A citation of a decision file by name that is not in `docs/decisions/`.
    Named { file: PathBuf, line: usize, name: String },
    /// A backticked path of a crate or directory that does not exist.
    Missing { file: PathBuf, line: usize, path: String },
    /// A decision file whose front matter is wrong.
    Front { file: PathBuf, defect: Defect },
    /// A file in `docs/decisions/` that is not `NNNN-slug.md`.
    Shape { file: PathBuf },
    /// A decision whose front matter number is not the one of its file name.
    Misnumbered { file: PathBuf, found: Number },
    /// A decision number over the ceiling in `.config/layout.toml`.
    Ceiling { file: PathBuf, number: Number },
    /// Two files with one number.
    Twice { number: Number },
    /// A number below the highest that has no file, no reservation and no gap row.
    Gap { number: Number },
    /// A gap row for a number that has a file or a reservation.
    Mended { number: Number },
    /// A reserved number whose heading is not in its home.
    Lost { number: Number, home: PathBuf },
    /// A `supersedes` that names a number with no file.
    Ghost { file: PathBuf, number: Number },
    /// A decision some other supersedes, whose status is not `superseded`.
    Replaced { number: Number },
    /// A decision with status `superseded` that no other supersedes.
    Orphan { number: Number },
    /// The decisions index that is not what the front matter generates.
    Stale { path: PathBuf },
}

#[expect(clippy::too_many_lines, reason = "one arm per fault, each a sentence")]
impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Broken { file, line, target } => {
                write!(out, "{}:{line}: the link to `{target}` names no path", file.display())
            }
            Self::Cite { file, line, number } => {
                write!(out, "{}:{line}: cites decision {number}, which has no file and is not reserved", file.display())
            }
            Self::Named { file, line, name } => {
                write!(
                    out,
                    "{}:{line}: cites `decisions/{name}`, which is not a file of docs/decisions/",
                    file.display()
                )
            }
            Self::Missing { file, line, path } => write!(
                out,
                "{}:{line}: `{path}` does not exist; mark the sentence planned, or fix the path",
                file.display()
            ),
            Self::Front { file, defect } => write!(out, "{}: {defect}", file.display()),
            Self::Shape { file } => write!(out, "{} is not a file named `NNNN-slug.md`", file.display()),
            Self::Misnumbered { file, found } => {
                write!(out, "{}: the front matter says {found}, the file name another number", file.display())
            }
            Self::Ceiling { file, number } => {
                write!(out, "{}: number {number} is over `decisions.ceiling` in .config/layout.toml", file.display())
            }
            Self::Twice { number } => write!(out, "two decision files are numbered {number}"),
            Self::Gap { number } => write!(
                out,
                "no decision {number}: no file, no reservation and no `decisions.gap` row with a reason in .config/layout.toml"
            ),
            Self::Mended { number } => {
                write!(out, "a `decisions.gap` row names {number}, which now has a file or a reservation; remove it")
            }
            Self::Lost { number, home } => {
                write!(out, "{number} is reserved, but {} has no `## {number}` heading", home.display())
            }
            Self::Ghost { file, number } => write!(out, "{}: supersedes {number}, which has no file", file.display()),
            Self::Replaced { number } => {
                write!(out, "{number} is superseded by another decision but its status is not")
            }
            Self::Orphan { number } => write!(out, "{number} has status superseded but no decision supersedes it"),
            Self::Stale { path } => {
                write!(out, "{} is not what the front matter generates (run `cargo xtask docs`)", path.display())
            }
        }
    }
}

impl Error for Fault {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Self::Front { defect, .. } = self { Some(defect) } else { None }
    }
}
