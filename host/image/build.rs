//! Hands the crate `INK_IMAGE_CODECS`: every registry crate it reaches through `Cargo.lock`'s
//! dependency graph, direct or not, with the version the lock holds. The image cache key hashes
//! it, so bumping a codec changes every key and nobody keeps a list of versions by hand.

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use toml::{Table, Value};

/// Why the lock could not be read.
#[derive(Debug)]
enum Failure {
    /// A file could not be read.
    Read(PathBuf, std::io::Error),
    /// A file is not TOML.
    Parse(PathBuf, toml::de::Error),
    /// No `Cargo.lock` in this directory or above it.
    NoLock(PathBuf),
    /// The lock has no package of this name.
    NoPackage(String),
    /// The lock names a dependency it holds no package for.
    NoVersion(String),
    /// Cargo did not say where the crate is (`CARGO_MANIFEST_DIR` is unset).
    NoManifestDir,
    /// A manifest has no `[package] name`.
    NoName(PathBuf),
    /// The lines for cargo could not be written to standard output.
    Write(std::io::Error),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(path, error) => write!(out, "{} could not be read: {error}", path.display()),
            Self::Parse(path, error) => write!(out, "{} is not TOML: {error}", path.display()),
            Self::NoLock(dir) => write!(out, "no Cargo.lock in {} or above it", dir.display()),
            Self::NoPackage(name) => write!(out, "Cargo.lock holds no package {name}"),
            Self::NoVersion(entry) => write!(out, "Cargo.lock names {entry} and holds no package for it"),
            Self::NoManifestDir => out.write_str("CARGO_MANIFEST_DIR is not set"),
            Self::NoName(path) => write!(out, "{} has no [package] name", path.display()),
            Self::Write(error) => write!(out, "cargo lines could not be written: {error}"),
        }
    }
}

/// The packages of a `Cargo.lock`.
struct Lock {
    packages: Vec<Table>,
}

impl Lock {
    /// The lock of the workspace holding `dir`, found by looking upward.
    fn find(dir: &Path) -> Result<(PathBuf, Self), Failure> {
        let path = dir
            .ancestors()
            .map(|ancestor| ancestor.join("Cargo.lock"))
            .find(|candidate| candidate.is_file())
            .ok_or_else(|| Failure::NoLock(dir.to_path_buf()))?;
        let text = std::fs::read_to_string(&path).map_err(|error| Failure::Read(path.clone(), error))?;
        let table: Table = text.parse().map_err(|error| Failure::Parse(path.clone(), error))?;
        let packages = match table.get("package") {
            Some(Value::Array(packages)) => packages.iter().filter_map(|package| package.as_table().cloned()).collect(),
            _ => Vec::new(),
        };
        Ok((path, Self { packages }))
    }

    /// A package by name, and by version when the lock holds several.
    fn package(&self, name: &str, version: Option<&str>) -> Option<&Table> {
        self.packages.iter().find(|package| {
            package.get("name").and_then(Value::as_str) == Some(name)
                && version.is_none_or(|wanted| package.get("version").and_then(Value::as_str) == Some(wanted))
        })
    }

    /// `name version` of every registry crate `package` reaches through the lock's dependency
    /// graph, however deep (a git crate also names its source), so a bump anywhere under a codec
    /// changes the key.
    fn codecs(&self, package: &str) -> Result<Vec<String>, Failure> {
        let root = self.package(package, None).ok_or_else(|| Failure::NoPackage(package.to_owned()))?;
        let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
        let mut codecs: BTreeSet<String> = BTreeSet::new();
        let mut pending = vec![root];
        while let Some(next) = pending.pop() {
            let listed = next.get("dependencies").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
            for entry in listed.iter().filter_map(Value::as_str) {
                let mut words = entry.split(' ');
                let name = words.next().unwrap_or_default();
                let found = self.package(name, words.next()).ok_or_else(|| Failure::NoVersion(entry.to_owned()))?;
                let held = found.get("version").and_then(Value::as_str).unwrap_or_default();
                if !seen.insert((name.to_owned(), held.to_owned())) {
                    continue;
                }
                if let Some(source) = found.get("source").and_then(Value::as_str) {
                    codecs.insert(if source.starts_with("git+") {
                        format!("{name} {held} {source}")
                    } else {
                        format!("{name} {held}")
                    });
                }
                pending.push(found);
            }
        }
        Ok(codecs.into_iter().collect())
    }
}

/// The `[package] name` of a manifest.
struct Manifest;

impl Manifest {
    /// The name of the package in `dir/Cargo.toml`.
    fn name(dir: &Path) -> Result<String, Failure> {
        let path = dir.join("Cargo.toml");
        let text = std::fs::read_to_string(&path).map_err(|error| Failure::Read(path.clone(), error))?;
        let table: Table = text.parse().map_err(|error| Failure::Parse(path.clone(), error))?;
        table
            .get("package")
            .and_then(Value::as_table)
            .and_then(|package| package.get("name"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or(Failure::NoName(path))
    }
}

/// The line the crate reads its codecs from.
struct Codecs;

impl Codecs {
    /// Writes `INK_IMAGE_CODECS` and what it depends on for cargo.
    fn emit() -> Result<(), Failure> {
        let dir = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from).ok_or(Failure::NoManifestDir)?;
        let name = Manifest::name(&dir)?;
        let (path, lock) = Lock::find(&dir)?;
        let codecs = lock.codecs(&name)?.join(" ");
        let mut out = std::io::stdout().lock();
        let written = writeln!(out, "cargo:rerun-if-changed={}", path.display())
            .and_then(|()| writeln!(out, "cargo:rerun-if-changed=Cargo.toml"))
            .and_then(|()| writeln!(out, "cargo:rustc-env=INK_IMAGE_CODECS={codecs}"));
        written.map_err(Failure::Write)
    }
}

fn main() -> ExitCode {
    match Codecs::emit() {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            if let Err(closed) = writeln!(std::io::stderr(), "{failure}") {
                // stderr is closed too: the failing exit code is the only report left
                drop(closed);
            }
            ExitCode::FAILURE
        }
    }
}
