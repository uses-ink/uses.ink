//! The fixture harness: every golden of a crate's `fixture/` directory compared through `insta`,
//! failing loudly (docs/conventions.md, Tests).
//!
//! [`files`] runs each `<name>.<extension>` input through a closure; [`cases`] takes outputs
//! built in code. Either way the output named `name` is compared with `<name>.snap`, and the run
//! fails, listing every reason at once, when:
//!
//! - there is nothing to compare (an empty suite);
//! - an input has no golden (outside update mode);
//! - a golden has no input, or a stray `.snap.new` or `.pending-snap` lies beside them;
//! - an output differs from its golden (insta prints the diff);
//! - in update mode (`INSTA_UPDATE=always` or `=new`), a golden was written: the run lists
//!   which, so an update never passes unnoticed. Read them, then run again without it.
//!
//! A directory is relative to the crate under test, which is where cargo and nextest run its
//! tests (and what the wasm32-wasip2 runner maps to `/`).
//!
//! ```no_run
//! #[test]
//! fn fixtures() {
//!     uses_ink_fixture::files("fixture", "md", |_path, bytes| String::from_utf8_lossy(bytes).to_uppercase());
//! }
//! ```

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

/// The extension of a golden.
const GOLDEN: &str = "snap";

/// Files insta leaves while an update waits for review.
const PENDING: &[&str] = &[".snap.new", ".pending-snap"];

/// Runs every `<name>.<extension>` file in `dir` through `render` (given its path and bytes) and
/// compares the output with `<name>.snap`. Panics with every failure listed.
pub fn files(dir: impl AsRef<Path>, extension: &str, mut render: impl FnMut(&Path, &[u8]) -> String) {
    let mut suite = Suite::new(dir.as_ref());
    for path in suite.entries() {
        let Some(name) = path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(extension)) else {
            continue;
        };
        let Some(name) = name.strip_suffix('.').filter(|name| !name.is_empty()) else { continue };
        match std::fs::read(&path) {
            Ok(bytes) => {
                let output = render(&path, &bytes);
                suite.compare(name, Some(&path), &output);
            }
            Err(source) => suite.failures.push(Failure::Read { path: path.clone(), source }),
        }
    }
    suite.finish();
}

/// Compares each named output built in code with `<name>.snap` in `dir`. Panics with every
/// failure listed.
pub fn cases<N: AsRef<str>>(dir: impl AsRef<Path>, cases: impl IntoIterator<Item = (N, String)>) {
    let mut suite = Suite::new(dir.as_ref());
    for (name, output) in cases {
        suite.compare(name.as_ref(), None, &output);
    }
    suite.finish();
}

/// One run over a fixture directory.
#[derive(Debug)]
struct Suite {
    /// The directory, absolute.
    dir: PathBuf,
    update: bool,
    /// The names compared so far.
    names: Vec<String>,
    failures: Vec<Failure>,
}

impl Suite {
    fn new(dir: &Path) -> Self {
        let update = std::env::var("INSTA_UPDATE").is_ok_and(|mode| matches!(mode.as_str(), "always" | "new"));
        let mut failures = Vec::new();
        let dir = match std::env::current_dir() {
            Ok(here) => here.join(dir),
            Err(source) => {
                failures.push(Failure::Read { path: PathBuf::from("."), source });
                dir.to_path_buf()
            }
        };
        let mut suite = Self { dir, update, names: Vec::new(), failures };
        let pending: Vec<PathBuf> = suite
            .entries()
            .into_iter()
            .filter(|path| path.to_str().is_some_and(|p| PENDING.iter().any(|suffix| p.ends_with(suffix))))
            .collect();
        suite.failures.extend(pending.into_iter().map(Failure::Pending));
        suite
    }

    /// The files in the directory, sorted; a directory that cannot be read is a failure.
    fn entries(&mut self) -> Vec<PathBuf> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(source) => {
                self.failures.push(Failure::Read { path: self.dir.clone(), source });
                return Vec::new();
            }
        };
        let mut paths = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) if entry.file_type().is_ok_and(|kind| kind.is_file()) => paths.push(entry.path()),
                Ok(_) => {}
                Err(source) => self.failures.push(Failure::Read { path: self.dir.clone(), source }),
            }
        }
        paths.sort();
        paths
    }

    fn golden(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.{GOLDEN}"))
    }

    /// Compares `output` with the golden of `name` through insta.
    fn compare(&mut self, name: &str, input: Option<&Path>, output: &str) {
        if self.names.iter().any(|n| n == name) {
            self.failures.push(Failure::Twice(name.to_owned()));
            return;
        }
        self.names.push(name.to_owned());
        let golden = self.golden(name);
        if !self.update && !golden.is_file() {
            self.failures.push(Failure::Missing(golden));
            return;
        }
        let before = Written::of(&golden);
        let mut settings = insta::Settings::clone_current();
        settings.set_snapshot_path(&self.dir);
        settings.set_prepend_module_to_snapshot(false);
        settings.set_omit_expression(true);
        settings.remove_snapshot_suffix();
        if let Some(input) = input {
            settings.set_input_file(input);
        }
        // insta prints the diff and panics on a difference; the panic is one failure of many.
        let same = catch_unwind(AssertUnwindSafe(|| settings.bind(|| insta::assert_snapshot!(name, output))));
        if same.is_err() {
            self.failures.push(Failure::Differs(golden.clone()));
        }
        if self.update && Written::of(&golden) != before {
            self.failures.push(Failure::Updated(golden));
        }
    }

    /// Checks for goldens without an input and for an empty suite, then panics if anything
    /// failed.
    fn finish(mut self) {
        let orphans: Vec<PathBuf> = self
            .entries()
            .into_iter()
            .filter(|path| path.extension().is_some_and(|e| e == GOLDEN))
            .filter(|path| {
                let name = path.file_stem().and_then(|n| n.to_str());
                !name.is_some_and(|name| self.names.iter().any(|n| n == name))
            })
            .collect();
        self.failures.extend(orphans.into_iter().map(Failure::Orphan));
        if self.names.is_empty() {
            self.failures.push(Failure::Empty(self.dir.clone()));
        }
        assert!(self.failures.is_empty(), "{}", Report(&self.failures));
    }
}

/// What is on disk for a golden: its bytes, and whether insta left a pending `.snap.new` for it.
#[derive(Debug, PartialEq, Eq)]
struct Written {
    golden: Option<Vec<u8>>,
    pending: Option<Vec<u8>>,
}

impl Written {
    fn of(golden: &Path) -> Self {
        let pending = golden.with_extension("snap.new");
        Self { golden: std::fs::read(golden).ok(), pending: std::fs::read(pending).ok() }
    }
}

/// One reason a fixture run fails.
#[derive(Debug)]
enum Failure {
    /// Nothing was compared.
    Empty(PathBuf),
    /// An output with no golden, outside update mode.
    Missing(PathBuf),
    /// A golden with no input or case.
    Orphan(PathBuf),
    /// An update waiting for review.
    Pending(PathBuf),
    /// Two outputs with one name.
    Twice(String),
    /// An output that differs from its golden.
    Differs(PathBuf),
    /// A golden written in update mode.
    Updated(PathBuf),
    /// A file or directory that could not be read.
    Read { path: PathBuf, source: std::io::Error },
}

impl fmt::Display for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty(dir) => write!(out, "{}: no inputs, so nothing was compared", dir.display()),
            Self::Missing(golden) => {
                write!(out, "{}: no golden; write it with INSTA_UPDATE=new and read it", golden.display())
            }
            Self::Orphan(golden) => write!(out, "{}: a golden without an input", golden.display()),
            Self::Pending(path) => {
                write!(out, "{}: an update waiting for review; accept or delete it", path.display())
            }
            Self::Twice(name) => write!(out, "`{name}`: two outputs share the name"),
            Self::Differs(golden) => write!(out, "{}: the output differs (diff above)", golden.display()),
            Self::Updated(golden) => {
                write!(out, "{}: written in update mode; read it, then run without INSTA_UPDATE", golden.display())
            }
            Self::Read { path, source } => write!(out, "{}: cannot be read: {source}", path.display()),
        }
    }
}

/// Every failure of a run, a line each.
struct Report<'a>(&'a [Failure]);

impl fmt::Display for Report<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{} fixture failure(s):", self.0.len())?;
        self.0.iter().try_for_each(|failure| write!(out, "\n  {failure}"))
    }
}
