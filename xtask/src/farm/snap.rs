//! The snapshot files a bless brings back. A host lists them; nothing it lists is trusted: a
//! path is relative, stays inside the tree, and ends in `.snap`, so a leftover `.snap.new` or a
//! source file is never copied.

use std::fmt;

/// Paths of changed `*.snap` files, checked, relative to the workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Snaps(Vec<String>);

impl Snaps {
    /// Reads the lines of `find`'s output (`./a/b.snap`).
    pub(crate) fn read(lines: &[String]) -> Result<Self, Fault> {
        let mut paths = Vec::new();
        for line in lines.iter().filter(|line| !line.is_empty()) {
            let path = line.strip_prefix("./").unwrap_or(line);
            let bad = |why| Fault { line: line.clone(), why };
            if path.starts_with('/') {
                return Err(bad(Why::Absolute));
            }
            if path.split('/').any(|part| part == ".." || part.is_empty()) {
                return Err(bad(Why::Escapes));
            }
            if path.chars().any(char::is_control) {
                return Err(bad(Why::Control));
            }
            if std::path::Path::new(path).extension().is_none_or(|ext| ext != "snap") {
                return Err(bad(Why::Kind));
            }
            paths.push(path.to_owned());
        }
        paths.sort();
        paths.dedup();
        Ok(Self(paths))
    }

    pub(crate) fn paths(&self) -> &[String] {
        &self.0
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// One path per line, as `rsync --files-from` reads them.
    pub(crate) fn list(&self) -> String {
        self.0.iter().fold(String::new(), |mut list, path| {
            list.push_str(path);
            list.push('\n');
            list
        })
    }
}

/// A listed path that is not safe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fault {
    line: String,
    why: Why,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Why {
    Absolute,
    Escapes,
    Control,
    Kind,
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let line = &self.line;
        match self.why {
            Why::Absolute => write!(out, "`{line}` is absolute"),
            Why::Escapes => write!(out, "`{line}` leaves the tree or has an empty part"),
            Why::Control => write!(out, "`{line}` holds a control character"),
            Why::Kind => write!(out, "`{line}` is not a `.snap` file"),
        }
    }
}

impl std::error::Error for Fault {}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(lines: &[&str]) -> Result<Snaps, Fault> {
        Snaps::read(&lines.iter().map(|line| (*line).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn changed_snapshots_are_sorted_and_relative() {
        let snaps = read(&["./b/s/y.snap", "./a/x.snap", "./a/x.snap", ""]).unwrap();
        assert_eq!(snaps.paths(), ["a/x.snap", "b/s/y.snap"]);
        assert_eq!(snaps.list(), "a/x.snap\nb/s/y.snap\n");
        assert!(read(&[]).unwrap().is_empty());
    }

    #[test]
    fn nothing_else_is_ever_copied() {
        for bad in ["./a/x.snap.new", "./src/lib.rs", "/etc/x.snap", "./../x.snap", "./a/../../x.snap", "./a//x.snap", "./a\tb.snap", "./x.pending-snap"] {
            assert!(read(&[bad]).is_err(), "{bad:?}");
        }
    }
}
