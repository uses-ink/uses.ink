//! Generators and where their files go: written, or compared with what is on disk. A task's
//! outcome is a [`Report`].

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use proc_macro2::TokenStream;

use crate::Task;
use crate::error::{Error, Flaw};
use crate::process::Process;

/// A task that writes generated files from one source file, read as a typed source.
pub(crate) trait Generator {
    /// The task that runs it, named in the header of every file it writes.
    const TASK: Task;
    /// The file it reads, relative to the root.
    const SOURCE: &'static str;
    /// The source, read and checked: a generator never sees a source it cannot generate from.
    type Source: TryFrom<toml::Table, Error: Into<Flaw>>;

    /// Puts every file generated from `source`.
    fn generate(&self, source: &Self::Source, files: &mut Files<'_>) -> Result<(), Error>;
}

/// A list of generators, run in order. Implemented for tuples of [`Generator`]s.
pub(crate) trait Generators {
    /// Runs each into `output`, every failure into `report`.
    fn generate(&self, output: &mut Output, report: &mut Report);
}

tuples! { Generators: Generator, fn generate(&self, output: &mut Output, report: &mut Report) { |generator| report.take(output.generate(generator)) } }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Write each file.
    Write,
    /// Compare each file with the one on disk, writing nothing.
    Check,
}

/// The files generators produced, by path relative to the root.
#[derive(Debug)]
pub(crate) struct Output {
    root: PathBuf,
    mode: Mode,
    written: Vec<PathBuf>,
    removed: Vec<PathBuf>,
    stale: Vec<Stale>,
    /// Every path put, so a claimed directory's leftovers can be found.
    put: Vec<PathBuf>,
}

impl Output {
    pub(crate) fn new(root: impl Into<PathBuf>, mode: Mode) -> Self {
        Self { root: root.into(), mode, written: Vec::new(), removed: Vec::new(), stale: Vec::new(), put: Vec::new() }
    }

    /// Reads `G`'s source and puts every file it generates.
    pub(crate) fn generate<G: Generator>(&mut self, generator: &G) -> Result<(), Error> {
        let path = self.root.join(G::SOURCE);
        let text = std::fs::read_to_string(&path).map_err(|source| Error::Read { path, source })?;
        let invalid = |flaw: Flaw| Error::Source { path: G::SOURCE, flaw };
        let table: toml::Table = toml::from_str(&text).map_err(|e| invalid(e.into()))?;
        let source = G::Source::try_from(table).map_err(|e| invalid(e.into()))?;
        let origin = Origin { task: G::TASK, source: G::SOURCE };
        generator.generate(&source, &mut Files { output: self, origin })
    }

    /// Adds what happened to `report`: the files written and removed, or every stale file as a
    /// failure.
    pub(crate) fn settle(self, report: &mut Report) {
        match self.mode {
            Mode::Check if self.stale.is_empty() => {}
            Mode::Check => report.fail(Error::Stale(self.stale)),
            Mode::Write if self.written.is_empty() && self.removed.is_empty() => report.line(Line::Unchanged),
            Mode::Write => {
                self.written
                    .into_iter()
                    .map(Line::Wrote)
                    .chain(self.removed.into_iter().map(Line::Removed))
                    .for_each(|line| report.line(line));
            }
        }
    }

    /// Runs `text` through rustfmt with the workspace's settings, so a generated file is what
    /// `cargo fmt` would leave.
    fn rustfmt(&self, path: &Path, text: &str) -> Result<String, Error> {
        let mut command = Command::new("rustfmt");
        command
            .args(["--edition", "2024", "--emit", "stdout", "--config-path"])
            .arg(self.root.join("rustfmt.toml"))
            .current_dir(&self.root);
        Process::from(command).text(text.as_bytes()).map_err(|source| Error::Rustfmt { path: path.into(), source })
    }
}

/// The files one generator puts, each recorded with the generator's origin.
#[derive(Debug)]
pub(crate) struct Files<'a> {
    output: &'a mut Output,
    origin: Origin,
}

impl Files<'_> {
    /// Where these files come from, for a header the generator writes itself.
    pub(crate) const fn origin(&self) -> Origin {
        self.origin
    }

    /// Puts `bytes` at `path` under the root.
    pub(crate) fn put(&mut self, path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> Result<(), Error> {
        let output = &mut *self.output;
        let relative = path.as_ref().to_path_buf();
        let path = output.root.join(&relative);
        output.put.push(relative.clone());
        let current = match std::fs::read(&path) {
            Ok(current) => Some(current),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(source) => return Err(Error::Read { path, source }),
        };
        if current.as_deref() == Some(bytes.as_ref()) {
            return Ok(());
        }
        match output.mode {
            Mode::Check => output.stale.push(Stale { task: self.origin.task, path: relative }),
            Mode::Write => {
                std::fs::write(&path, bytes).map_err(|source| Error::Write { path, source })?;
                output.written.push(relative);
            }
        }
        Ok(())
    }

    /// Puts generated Rust at `path`: printed by prettyplease, then rustfmt with the
    /// workspace's settings, so the file on disk is what `cargo fmt` would leave. The header
    /// says where it came from.
    pub(crate) fn rust(&mut self, path: impl AsRef<Path>, code: TokenStream) -> Result<(), Error> {
        let path = path.as_ref();
        let file: syn::File = syn::parse2(code).map_err(|source| Error::Generated { path: path.into(), source })?;
        let text = format!("// {}\n\n{}", self.origin, prettyplease::unparse(&file));
        let text = self.output.rustfmt(path, &text)?;
        self.put(path, text)
    }

    /// Claims every `.{extension}` file directly in `dir` as generated: one this run did not put
    /// is a leftover, removed when writing and stale when checking. Call after the puts.
    pub(crate) fn claim(&mut self, dir: impl AsRef<Path>, extension: &str) -> Result<(), Error> {
        let output = &mut *self.output;
        let dir = output.root.join(dir);
        let entries = std::fs::read_dir(&dir).map_err(|source| Error::Read { path: dir.clone(), source })?;
        for entry in entries {
            let entry = entry.map_err(|source| Error::Read { path: dir.clone(), source })?;
            let path = entry.path();
            let relative = path.strip_prefix(&output.root).unwrap_or(&path).to_path_buf();
            if path.extension().is_none_or(|e| e != extension) || output.put.contains(&relative) {
                continue;
            }
            match output.mode {
                Mode::Check => output.stale.push(Stale { task: self.origin.task, path: relative }),
                Mode::Write => {
                    std::fs::remove_file(&path).map_err(|source| Error::Write { path, source })?;
                    output.removed.push(relative);
                }
            }
        }
        Ok(())
    }
}

/// Where a generated file comes from: the task that writes it and the file it is written from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Origin {
    pub(crate) task: Task,
    pub(crate) source: &'static str,
}

impl Origin {
    /// How the sentence a generated file starts with begins, so a reader can tell one.
    pub(crate) const MARK: &'static str = "Generated by `cargo xtask ";
}

impl fmt::Display for Origin {
    /// The sentence a generated file starts with, inside a comment.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}{}` from `{}`. Edit that file, not this one.", Self::MARK, self.task, self.source)
    }
}

/// A generated file that differs from what its generator writes now.
#[derive(Debug)]
pub(crate) struct Stale {
    task: Task,
    path: PathBuf,
}

impl fmt::Display for Stale {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{} (run `cargo xtask {}`)", self.path.display(), self.task)
    }
}

/// What a task did: a line each for what went well, and every failure.
#[derive(Debug, Default)]
pub(crate) struct Report {
    lines: Vec<Line>,
    failures: Vec<Error>,
}

impl Report {
    /// Adds a line.
    pub(crate) fn line(&mut self, line: Line) {
        self.lines.push(line);
    }

    /// Adds a failure.
    pub(crate) fn fail(&mut self, error: Error) {
        self.failures.push(error);
    }

    /// Adds the failure of `result`, if it failed.
    pub(crate) fn take(&mut self, result: Result<(), Error>) {
        if let Err(error) = result {
            self.fail(error);
        }
    }

    pub(crate) fn lines(&self) -> &[Line] {
        &self.lines
    }

    pub(crate) fn failures(&self) -> &[Error] {
        &self.failures
    }
}

impl From<Error> for Report {
    fn from(error: Error) -> Self {
        Self { lines: Vec::new(), failures: vec![error] }
    }
}

/// One line of a task's report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Line {
    /// A generated file written, by path relative to the root.
    Wrote(PathBuf),
    /// A file in a claimed directory removed because nothing generates it any more.
    Removed(PathBuf),
    /// Every generated file already held what the generator writes.
    Unchanged,
    /// A crate breaking a rule that is not enforced on it yet, and how often.
    Waived { krate: String, rule: &'static str, count: usize },
    /// The layout and every generated file are as they should be.
    Checked,
    /// Every step of the gate passed.
    Passed,
}

impl fmt::Display for Line {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wrote(path) => write!(out, "wrote {}", path.display()),
            Self::Removed(path) => write!(out, "removed {} (no longer generated)", path.display()),
            Self::Unchanged => write!(out, "nothing changed"),
            Self::Waived { krate, rule, count } => {
                write!(out, "note: `{rule}` finds {count} in `{krate}`, not enforced there yet (xtask's ENFORCED)")
            }
            Self::Checked => write!(out, "layout fine, generated files fresh"),
            Self::Passed => write!(out, "check: every step passed"),
        }
    }
}
