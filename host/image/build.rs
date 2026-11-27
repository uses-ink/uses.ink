//! Hands the crate `INK_IMAGE_CODECS`: each registry crate it depends on, with the version
//! `Cargo.lock` holds. The image cache key hashes it, so bumping a codec changes every key and
//! nobody keeps a list of versions by hand.

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
}

impl std::fmt::Display for Failure {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(path, error) => write!(out, "{} could not be read: {error}", path.display()),
            Self::Parse(path, error) => write!(out, "{} is not TOML: {error}", path.display()),
            Self::NoLock(dir) => write!(out, "no Cargo.lock in {} or above it", dir.display()),
            Self::NoPackage(name) => write!(out, "Cargo.lock holds no package {name}"),
            Self::NoVersion(entry) => write!(out, "Cargo.lock names {entry} and holds no package for it"),
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

    /// `name version` of every registry crate `package` depends on and `wanted` lists.
    fn codecs(&self, package: &str, wanted: &[String]) -> Result<Vec<String>, Failure> {
        let mine = self.package(package, None).ok_or_else(|| Failure::NoPackage(package.to_owned()))?;
        let listed = mine.get("dependencies").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
        let mut codecs = Vec::new();
        for name in wanted {
            let Some(entry) =
                listed.iter().filter_map(Value::as_str).find(|entry| entry.split(' ').next() == Some(name))
            else {
                continue;
            };
            let version = entry.split(' ').nth(1);
            let found = self.package(name, version).ok_or_else(|| Failure::NoVersion(entry.to_owned()))?;
            if found.contains_key("source") {
                let held = found.get("version").and_then(Value::as_str).unwrap_or_default();
                codecs.push(format!("{name} {held}"));
            }
        }
        codecs.sort();
        Ok(codecs)
    }
}

/// The names under `[dependencies]` and `[package] name` of a manifest.
struct Manifest {
    name: String,
    dependencies: Vec<String>,
}

impl Manifest {
    /// `dir/Cargo.toml`, read.
    fn read(dir: &Path) -> Result<Self, Failure> {
        let path = dir.join("Cargo.toml");
        let text = std::fs::read_to_string(&path).map_err(|error| Failure::Read(path.clone(), error))?;
        let table: Table = text.parse().map_err(|error| Failure::Parse(path, error))?;
        let name = table
            .get("package")
            .and_then(Value::as_table)
            .and_then(|package| package.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let dependencies = table
            .get("dependencies")
            .and_then(Value::as_table)
            .map_or_else(Vec::new, |deps| deps.keys().cloned().collect());
        Ok(Self { name, dependencies })
    }
}

/// The line the crate reads its codecs from.
struct Codecs;

impl Codecs {
    /// Writes `INK_IMAGE_CODECS` and what it depends on for cargo.
    fn emit() -> Result<(), Failure> {
        let dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
        let manifest = Manifest::read(&dir)?;
        let (path, lock) = Lock::find(&dir)?;
        let codecs = lock.codecs(&manifest.name, &manifest.dependencies)?.join(" ");
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "cargo:rerun-if-changed={}", path.display());
        let _ = writeln!(out, "cargo:rerun-if-changed=Cargo.toml");
        let _ = writeln!(out, "cargo:rustc-env=INK_IMAGE_CODECS={codecs}");
        Ok(())
    }
}

fn main() -> ExitCode {
    match Codecs::emit() {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            let _ = writeln!(std::io::stderr(), "{failure}");
            ExitCode::FAILURE
        }
    }
}
