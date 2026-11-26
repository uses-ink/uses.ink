//! The workspace's layout rules (docs/conventions.md, Crates and directories): the path predicts
//! the name, dependencies flow one way, licences match the crate's kind, no banned names.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::error::Error;

/// Crates with a name of their own: product-independent, published as is, MIT OR Apache-2.0,
/// depending on no crate of ours.
const OWN_NAMES: &[&str] = &["freehand", "freehand-web"];

/// The repo's task runner: named as cargo expects, not published.
const TASKS: &str = "xtask";

/// Which crates of ours each crate may depend on (architecture.md section 4). A crate missing
/// here is a problem: its place in the order is decided when it is added.
const MAY_USE: &[(&str, &[&str])] = &[
    ("freehand", &[]),
    ("freehand-web", &["freehand"]),
    ("uses-ink-token", &[]),
    (TASKS, &["freehand", "uses-ink-token"]),
];

const PERMISSIVE: &str = "MIT OR Apache-2.0";
const SERVICE: &str = "AGPL-3.0-or-later";

/// Names that collect unrelated code.
const BANNED: &[&str] =
    &["util", "utils", "common", "core", "helpers", "misc", "base", "shared", "manager", "handler", "service", "impl"];

/// The only files that may hold unsafe code: the pen's wasm exports, whose names need the unsafe
/// `no_mangle` attribute. The workspace denies the unsafe-code lint rather than forbidding it so
/// these can expect it.
const UNSAFE: &[&str] = &["freehand-web/src/lib.rs"];

/// What unsafe code looks like in a source file: the keyword, then one of these. Kept apart so
/// this file does not match itself.
const KEYWORD: &str = "unsafe";
const UNSAFE_AFTER: &[&str] = &["_code", "(", " {", " fn", " impl", " extern", " trait"];

/// Directories not walked for banned names.
const SKIP: &[&str] = &["target", ".git", ".jj", ".impeccable", "node_modules"];

#[derive(Debug)]
pub(crate) enum Problem {
    Name { package: String, path: String, expected: String },
    Unordered { package: String },
    Dependency { package: String, on: String },
    Licence { package: String, found: Option<String>, expected: &'static str },
    Alias { package: String },
    Banned { path: PathBuf },
    Unsafe { path: PathBuf },
}

impl fmt::Display for Problem {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name { package, path, expected } => {
                write!(out, "`{package}` at `{path}/` should be named `{expected}`")
            }
            Self::Unordered { package } => {
                write!(out, "`{package}` has no row in xtask's MAY_USE: decide what it may depend on")
            }
            Self::Dependency { package, on } => write!(out, "`{package}` may not depend on `{on}`"),
            Self::Licence { package, found, expected } => {
                write!(
                    out,
                    "`{package}` is licensed `{}`, expected `{expected}`",
                    found.as_deref().unwrap_or("nothing")
                )
            }
            Self::Alias { package } => write!(out, "`{package}` has no alias in [workspace.dependencies]"),
            Self::Banned { path } => write!(out, "`{}` uses a banned name", path.display()),
            Self::Unsafe { path } => {
                write!(out, "`{}` holds unsafe code; only {} may (xtask's UNSAFE)", path.display(), UNSAFE.join(", "))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
    workspace_root: PathBuf,
}

#[derive(Debug, Deserialize)]
struct Package {
    id: String,
    name: String,
    manifest_path: PathBuf,
    license: Option<String>,
    dependencies: Vec<Dependency>,
}

#[derive(Debug, Deserialize)]
struct Dependency {
    name: String,
    path: Option<PathBuf>,
}

pub(crate) fn check(root: &Path) -> Result<(), Error> {
    let metadata = metadata(root)?;
    let members: Vec<&Package> =
        metadata.packages.iter().filter(|p| metadata.workspace_members.contains(&p.id)).collect();
    let ours: Vec<&str> = members.iter().map(|p| p.name.as_str()).collect();
    let manifest = std::fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|source| Error::Read { path: root.join("Cargo.toml"), source })?;
    let mut problems = Vec::new();
    let used: Vec<&str> = members.iter().flat_map(|p| p.dependencies.iter().map(|d| d.name.as_str())).collect();
    for package in &members {
        name(package, &metadata.workspace_root, &mut problems);
        dependencies(package, &ours, &metadata.workspace_root, &mut problems);
        licence(package, &mut problems);
        // a crate another member uses is named through the workspace's alias
        if used.contains(&package.name.as_str())
            && !manifest.contains(&format!("\"{}\"", package.name))
            && !manifest.contains(&format!("\n{} = {{ path", package.name))
        {
            problems.push(Problem::Alias { package: package.name.clone() });
        }
    }
    banned(&metadata.workspace_root, &metadata.workspace_root, &mut problems)?;
    for package in &members {
        let dir = package.manifest_path.parent().unwrap_or(&metadata.workspace_root);
        unsafety(dir, &metadata.workspace_root, &mut problems)?;
    }
    if problems.is_empty() { Ok(()) } else { Err(Error::Layout(problems)) }
}

fn metadata(root: &Path) -> Result<Metadata, Error> {
    let done = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps", "--offline"])
        .current_dir(root)
        .output()
        .map_err(|e| Error::Metadata { why: e.to_string() })?;
    if !done.status.success() {
        return Err(Error::Metadata { why: String::from_utf8_lossy(&done.stderr).into_owned() });
    }
    serde_json::from_slice(&done.stdout).map_err(|e| Error::Metadata { why: e.to_string() })
}

/// The directory of a crate, relative to the root, with `/` between parts.
fn place(package: &Package, root: &Path) -> String {
    let dir = package.manifest_path.parent().unwrap_or(root);
    let relative = dir.strip_prefix(root).unwrap_or(dir);
    relative.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
}

fn name(package: &Package, root: &Path, problems: &mut Vec<Problem>) {
    let path = place(package, root);
    let expected = if package.name == TASKS || OWN_NAMES.contains(&package.name.as_str()) {
        path.rsplit('/').next().unwrap_or(&path).to_owned()
    } else {
        // `plugin/theme/quiet` is `uses-ink-theme-quiet`: the slot directory drops.
        format!("uses-ink-{}", path.strip_prefix("plugin/").unwrap_or(&path).replace('/', "-"))
    };
    if package.name != expected {
        problems.push(Problem::Name { package: package.name.clone(), path, expected });
    }
}

fn dependencies(package: &Package, ours: &[&str], root: &Path, problems: &mut Vec<Problem>) {
    let order: BTreeMap<&str, &[&str]> = MAY_USE.iter().copied().collect();
    let Some(allowed) = order.get(package.name.as_str()) else {
        problems.push(Problem::Unordered { package: package.name.clone() });
        return;
    };
    for dependency in &package.dependencies {
        // A path dependency outside the workspace (a patched sibling) is not ours.
        let inside = dependency.path.as_ref().is_some_and(|p| p.starts_with(root));
        if inside && ours.contains(&dependency.name.as_str()) && !allowed.contains(&dependency.name.as_str()) {
            problems.push(Problem::Dependency { package: package.name.clone(), on: dependency.name.clone() });
        }
    }
}

fn licence(package: &Package, problems: &mut Vec<Problem>) {
    let expected = if OWN_NAMES.contains(&package.name.as_str()) { PERMISSIVE } else { SERVICE };
    if package.license.as_deref() != Some(expected) {
        problems.push(Problem::Licence { package: package.name.clone(), found: package.license.clone(), expected });
    }
}

fn banned(dir: &Path, root: &Path, problems: &mut Vec<Problem>) -> Result<(), Error> {
    let entries = std::fs::read_dir(dir).map_err(|source| Error::Read { path: dir.to_path_buf(), source })?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::Read { path: dir.to_path_buf(), source })?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if SKIP.contains(&name.as_str()) {
            continue;
        }
        let stem = name.split('.').next().unwrap_or(&name);
        if BANNED.contains(&stem) {
            problems.push(Problem::Banned { path: path.strip_prefix(root).unwrap_or(&path).to_path_buf() });
        }
        if path.is_dir() {
            banned(&path, root, problems)?;
        }
    }
    Ok(())
}

/// Every `.rs` file under `dir` that holds unsafe code without being listed in `UNSAFE`.
fn unsafety(dir: &Path, root: &Path, problems: &mut Vec<Problem>) -> Result<(), Error> {
    let entries = std::fs::read_dir(dir).map_err(|source| Error::Read { path: dir.to_path_buf(), source })?;
    for entry in entries {
        let entry = entry.map_err(|source| Error::Read { path: dir.to_path_buf(), source })?;
        let path = entry.path();
        if SKIP.contains(&entry.file_name().to_string_lossy().as_ref()) {
            continue;
        }
        if path.is_dir() {
            unsafety(&path, root, problems)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            let allowed = UNSAFE.iter().any(|a| relative == Path::new(a));
            let text = std::fs::read_to_string(&path).map_err(|source| Error::Read { path: path.clone(), source })?;
            let holds = text.match_indices(KEYWORD).any(|(at, _)| {
                text.get(at + KEYWORD.len()..)
                    .is_some_and(|rest| UNSAFE_AFTER.iter().any(|after| rest.starts_with(after)))
            });
            if !allowed && holds {
                problems.push(Problem::Unsafe { path: relative });
            }
        }
    }
    Ok(())
}
