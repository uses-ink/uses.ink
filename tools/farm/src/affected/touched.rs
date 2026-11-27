//! What changed in the workspace: a [`Base`] to measure from, the [`Touched`] paths since it, and
//! the [`Touch`] seam that lists them. [`Diff`] asks jj; tests give a fixed list.

use std::fmt;
use std::path::Path;
use std::process::Command;
use std::str::FromStr;

use serde::Deserialize;

use crate::affected::Fault;
use crate::host::transport::process::Process;

/// A revision the changes are measured from, as jj reads it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct Base(String);

impl Base {
    /// The parent of the workspace's first commit above `main`: where the workspace left it.
    const DEFAULT: &'static str = "heads(::main & ::@)";

    /// The revset text.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Base {
    fn default() -> Self {
        Self(Self::DEFAULT.to_owned())
    }
}

impl FromStr for Base {
    type Err = BadBase;

    fn from_str(given: &str) -> Result<Self, BadBase> {
        if given.is_empty() || given.starts_with('-') || given.chars().any(char::is_control) {
            return Err(BadBase { given: given.to_owned() });
        }
        Ok(Self(given.to_owned()))
    }
}

impl TryFrom<String> for Base {
    type Error = BadBase;

    fn try_from(given: String) -> Result<Self, BadBase> {
        given.parse()
    }
}

/// A base that is empty, starts with `-` or holds a control character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BadBase {
    pub(crate) given: String,
}

impl fmt::Display for BadBase {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`{}` is not a revision", self.given)
    }
}

impl std::error::Error for BadBase {}

/// A path that changed, relative to the workspace root with `/` between segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Touched(String);

impl Touched {
    /// Files at the root, and directories below it, that every crate depends on: a change to one
    /// of them touches every crate.
    const FILES: [&'static str; 5] = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "rust-toolchain", "deny.toml"];
    const DIRS: [&'static str; 2] = [".config", ".cargo"];

    /// A path as jj prints it; refused when it is absolute, leaves the tree or is not text.
    pub(crate) fn read(path: &str) -> Result<Self, Fault> {
        let unsafe_part = |part: &str| part.is_empty() || part == "..";
        if path.starts_with('/') || path.split('/').any(unsafe_part) || path.chars().any(char::is_control) {
            return Err(Fault::Path { path: path.to_owned() });
        }
        Ok(Self(path.to_owned()))
    }

    /// The path.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether the path is one every crate depends on.
    pub(crate) fn shared(&self) -> bool {
        Self::FILES.contains(&self.0.as_str())
            || Self::DIRS.iter().any(|dir| self.0.strip_prefix(dir).is_some_and(|rest| rest.starts_with('/')))
    }
}

impl fmt::Display for Touched {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// The seam to the version control: which paths changed since a base.
pub(crate) trait Touch {
    /// Every path added, modified, deleted or renamed (old and new) since `base`, the working
    /// copy included.
    fn paths(&self, base: &Base) -> Result<Vec<Touched>, Fault>;
}

/// jj's answer, for the workspace at `root`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Diff<'a> {
    pub(crate) root: &'a Path,
}

impl Touch for Diff<'_> {
    fn paths(&self, base: &Base) -> Result<Vec<Touched>, Fault> {
        let mut jj = Command::new("jj");
        jj.arg("-R")
            .arg(self.root)
            .args(["diff", "--no-pager", "--summary", "--from", base.as_str(), "--to", "@"])
            .current_dir(self.root);
        let text = Process::from(jj).text(&[]).map_err(|source| Fault::Diff { base: base.clone(), source })?;
        Listing::read(&text)
    }
}

/// The text of `jj diff --summary`: a letter and a path a line, `R` and `C` with `{old => new}`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Listing;

impl Listing {
    /// Every path the lines name; both sides of a rename or a copy.
    pub(crate) fn read(text: &str) -> Result<Vec<Touched>, Fault> {
        let mut found = Vec::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let bad = || Fault::Summary { line: line.to_owned() };
            let (kind, rest) = line.split_once(' ').ok_or_else(bad)?;
            match kind {
                "A" | "M" | "D" => found.push(Touched::read(rest)?),
                "R" | "C" => {
                    let (old, new) = Self::sides(rest).ok_or_else(bad)?;
                    found.push(Touched::read(&old)?);
                    found.push(Touched::read(&new)?);
                }
                _ => return Err(bad()),
            }
        }
        Ok(found)
    }

    /// `a/{b => c}/d` is `a/b/d` and `a/c/d`; `{ => x}/d` is `d` and `x/d`; `a => b` is both.
    fn sides(rest: &str) -> Option<(String, String)> {
        let Some((head, tail)) = rest.split_once('{') else {
            return rest.split_once(" => ").map(|(old, new)| (old.to_owned(), new.to_owned()));
        };
        let (middle, after) = tail.split_once('}')?;
        let (old, new) = middle.split_once(" => ")?;
        let join = |part: &str| format!("{head}{part}{after}").replace("//", "/");
        Some((join(old), join(new)))
    }
}
