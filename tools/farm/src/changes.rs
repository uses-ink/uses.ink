//! The files a verb brings back from a host: the `*.snap` files a bless changed, the `.rs` files
//! a fmt changed. A host lists them; nothing it lists is trusted: a path is relative, stays
//! inside the tree, and ends in the extension of the [`Kind`] asked for, so a leftover
//! `.snap.new` or a source file a bless never touches is never copied.

use std::fmt;
use std::path::Path;
use std::time::SystemTime;

/// What a verb brings back, by file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// `*.snap`, from a bless.
    Snap,
    /// `*.rs`, from a fmt.
    Rust,
}

impl Kind {
    /// The extension, without the dot.
    pub(crate) const fn ext(self) -> &'static str {
        match self {
            Self::Snap => "snap",
            Self::Rust => "rs",
        }
    }

    /// The `find -name` pattern of its files.
    pub(crate) fn pattern(self) -> String {
        format!("*.{}", self.ext())
    }
}

/// Paths of changed files of one kind, checked, relative to the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Changes {
    paths: Vec<String>,
}

impl Changes {
    /// Reads the lines of `find`'s output (`./a/b.snap`).
    pub(crate) fn read(kind: Kind, lines: &[String]) -> Result<Self, Fault> {
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
            if Path::new(path).extension().is_none_or(|ext| ext != kind.ext()) {
                return Err(bad(Why::Kind(kind)));
            }
            paths.push(path.to_owned());
        }
        paths.sort();
        paths.dedup();
        Ok(Self { paths })
    }

    pub(crate) fn paths(&self) -> &[String] {
        &self.paths
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// One path per line, as `rsync --files-from` reads them.
    pub(crate) fn list(&self) -> String {
        self.paths.iter().fold(String::new(), |mut list, path| {
            list.push_str(path);
            list.push('\n');
            list
        })
    }

    /// The listed files under `root` that were edited after `since`, or are gone: bringing the
    /// host's copy back would overwrite work that the host never saw.
    pub(crate) fn edited(&self, root: &Path, since: SystemTime) -> Result<Vec<String>, std::io::Error> {
        let mut edited = Vec::new();
        for path in &self.paths {
            match root.join(path).metadata().and_then(|meta| meta.modified()) {
                Ok(at) if at <= since => {}
                Ok(_) => edited.push(path.clone()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => edited.push(path.clone()),
                Err(error) => return Err(error),
            }
        }
        Ok(edited)
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
    Kind(Kind),
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let line = &self.line;
        match self.why {
            Why::Absolute => write!(out, "`{line}` is absolute"),
            Why::Escapes => write!(out, "`{line}` leaves the tree or has an empty part"),
            Why::Control => write!(out, "`{line}` holds a control character"),
            Why::Kind(kind) => write!(out, "`{line}` is not a `.{}` file", kind.ext()),
        }
    }
}

impl std::error::Error for Fault {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn read(kind: Kind, lines: &[&str]) -> Result<Changes, Fault> {
        Changes::read(kind, &lines.iter().map(|line| (*line).to_owned()).collect::<Vec<_>>())
    }

    #[test]
    fn changed_snapshots_are_sorted_and_relative() {
        let snaps = read(Kind::Snap, &["./b/s/y.snap", "./a/x.snap", "./a/x.snap", ""]).unwrap();
        assert_eq!(snaps.paths(), ["a/x.snap", "b/s/y.snap"]);
        assert_eq!(snaps.list(), "a/x.snap\nb/s/y.snap\n");
        assert!(read(Kind::Snap, &[]).unwrap().is_empty());
    }

    #[test]
    fn nothing_else_is_ever_copied() {
        for bad in [
            "./a/x.snap.new",
            "./src/lib.rs",
            "/etc/x.snap",
            "./../x.snap",
            "./a/../../x.snap",
            "./a//x.snap",
            "./a\tb.snap",
            "./x.pending-snap",
        ] {
            assert!(read(Kind::Snap, &[bad]).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_fmt_brings_back_rust_files_and_nothing_else() {
        let rust = read(Kind::Rust, &["./src/b.rs", "./a.rs"]).unwrap();
        assert_eq!(rust.paths(), ["a.rs", "src/b.rs"]);
        assert_eq!(Kind::Rust.pattern(), "*.rs");
        for bad in ["./a.snap", "./Cargo.toml", "./a.rs.bk", "./../a.rs", "/a.rs"] {
            assert!(read(Kind::Rust, &[bad]).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn a_file_edited_after_the_start_or_gone_is_never_overwritten() {
        let base = std::env::temp_dir().join(format!("farm-edited-{}", std::process::id()));
        std::fs::remove_dir_all(&base).ok();
        std::fs::create_dir_all(&base).unwrap();
        let (calm, busy) = (base.join("calm.rs"), base.join("busy.rs"));
        std::fs::write(&calm, "a").unwrap();
        std::fs::write(&busy, "b").unwrap();
        let since = SystemTime::now() - Duration::from_secs(60);
        let before = since - Duration::from_secs(60);
        std::fs::File::options().write(true).open(&calm).unwrap().set_modified(before).unwrap();
        let changes = read(Kind::Rust, &["./calm.rs", "./busy.rs", "./gone.rs"]).unwrap();
        assert_eq!(changes.edited(&base, since).unwrap(), ["busy.rs", "gone.rs"]);
        std::fs::remove_dir_all(&base).unwrap();
    }
}
