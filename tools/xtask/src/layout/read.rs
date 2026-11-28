//! Reads the [`Workspace`]: `cargo metadata`, the `[workspace.dependencies]` aliases, a walk of the
//! tree, and the module tree each target compiles.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use super::config::Config;
use super::model::{Attributes, Crate, Dependency, Kind, Module, Role, Source, Workspace};
use crate::error::Error;
use crate::generate::output::Origin;
use crate::process::Process;

impl Workspace {
    /// Directories not walked.
    pub(crate) const SKIP: &'static [&'static str] = &["target", ".git", ".jj", ".impeccable", "node_modules"];

    /// Reads the workspace whose root manifest is under `root`.
    pub(crate) fn read(root: impl AsRef<Path>) -> Result<Self, Error> {
        let metadata = Metadata::read(root.as_ref())?;
        let root = metadata.workspace_root.as_path();
        let config = Config::read(root)?;
        let aliased = Self::aliased(root)?;
        let mut entries = Vec::new();
        Self::walk(root, root, &mut entries)?;
        let packages: Vec<&Package> =
            metadata.packages.iter().filter(|p| metadata.workspace_members.contains(&p.id)).collect();
        let dirs: Vec<PathBuf> = packages.iter().map(|p| p.dir(root)).collect();
        let crates = packages.iter().map(|package| package.read(root, &dirs, &entries)).collect::<Result<_, _>>()?;
        Ok(Self { root: root.to_path_buf(), crates, aliased, entries, config })
    }

    /// The crates `[workspace.dependencies]` names.
    fn aliased(root: &Path) -> Result<Vec<String>, Error> {
        let path = root.join("Cargo.toml");
        let text = std::fs::read_to_string(&path).map_err(|source| Error::Read { path: path.clone(), source })?;
        let manifest: toml::Table = toml::from_str(&text).map_err(|source| Error::Manifest { path, source })?;
        let dependencies =
            manifest.get("workspace").and_then(|w| w.get("dependencies")).and_then(toml::Value::as_table);
        Ok(dependencies
            .into_iter()
            .flatten()
            .map(|(key, value)| value.get("package").and_then(toml::Value::as_str).unwrap_or(key).to_owned())
            .collect())
    }

    /// Every path under `dir`, relative to `root`; a symlinked directory is never followed.
    pub(crate) fn walk(dir: &Path, root: &Path, entries: &mut Vec<PathBuf>) -> Result<(), Error> {
        let read = |source| Error::Read { path: dir.to_path_buf(), source };
        let mut found = std::fs::read_dir(dir).map_err(read)?.collect::<Result<Vec<_>, _>>().map_err(read)?;
        found.sort_by_key(std::fs::DirEntry::file_name);
        for entry in found {
            let path = entry.path();
            if Self::SKIP.contains(&entry.file_name().to_string_lossy().as_ref()) {
                continue;
            }
            entries.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
            // the entry's own type, not its target's
            if entry.file_type().map_err(|source| Error::Read { path: path.clone(), source })?.is_dir() {
                Self::walk(&path, root, entries)?;
            }
        }
        Ok(())
    }
}

/// What `cargo metadata` says, as far as the rules need it.
#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<Package>,
    workspace_members: Vec<String>,
    workspace_root: PathBuf,
}

impl Metadata {
    fn read(root: &Path) -> Result<Self, Error> {
        let mut command = Command::new(env!("CARGO"));
        command.args(["metadata", "--format-version", "1", "--no-deps", "--offline", "--locked"]).current_dir(root);
        Process::from(command).json().map_err(Error::Metadata)
    }
}

#[derive(Debug, Deserialize)]
struct Package {
    id: String,
    name: String,
    manifest_path: PathBuf,
    license: Option<String>,
    dependencies: Vec<Declared>,
    targets: Vec<Target>,
}

/// A dependency as `cargo metadata` declares it.
#[derive(Debug, Deserialize)]
struct Declared {
    name: String,
    path: Option<PathBuf>,
    kind: Option<Kind>,
}

#[derive(Debug, Deserialize)]
struct Target {
    kind: Vec<String>,
    src_path: PathBuf,
}

impl Target {
    /// The role of its root: test code for tests and benches, an entry otherwise.
    fn role(&self) -> Role {
        if self.kind.iter().any(|k| k == "test" || k == "bench") { Role::Test } else { Role::Entry }
    }
}

impl Package {
    /// Its directory, relative to `root`.
    fn dir(&self, root: &Path) -> PathBuf {
        let dir = self.manifest_path.parent().unwrap_or(root);
        dir.strip_prefix(root).unwrap_or(dir).to_path_buf()
    }

    /// The crate, with its files (`entries` under its directory and no nested crate's, `dirs`)
    /// read and its modules parsed.
    fn read(&self, root: &Path, dirs: &[PathBuf], entries: &[PathBuf]) -> Result<Crate, Error> {
        let dir = self.dir(root);
        // the deepest crate directory holding a path owns it
        let owner = |path: &Path| dirs.iter().filter(|d| path.starts_with(d)).max_by_key(|d| d.components().count());
        let files = entries
            .iter()
            .filter(|path| path.extension().is_some_and(|e| e == "rs") && owner(path) == Some(&dir))
            .filter(|path| root.join(path).is_file())
            .map(|path| {
                let full = root.join(path);
                let text = std::fs::read_to_string(&full).map_err(|source| Error::Read { path: full, source })?;
                Ok(Source { path: path.clone(), text })
            })
            .collect::<Result<_, Error>>()?;
        let mut tree = Tree { root, found: BTreeMap::new() };
        for target in &self.targets {
            let path = target.src_path.strip_prefix(root).unwrap_or(&target.src_path).to_path_buf();
            tree.file(Spot { path: &path, role: target.role(), nested: path.parent().unwrap_or(Path::new("")) })?;
        }
        let dependencies = self
            .dependencies
            .iter()
            .map(|d| Dependency {
                name: d.name.clone(),
                inside: d.path.as_ref().is_some_and(|p| p.starts_with(root)),
                kind: d.kind,
            })
            .collect();
        Ok(Crate {
            name: self.name.clone(),
            dir,
            licence: self.license.clone(),
            dependencies,
            files,
            modules: tree.found.into_values().collect(),
        })
    }
}

/// Where a module's file sits in a crate's module tree.
#[derive(Debug, Clone, Copy)]
struct Spot<'p> {
    /// The file, relative to the workspace root.
    path: &'p Path,
    /// What the modules it declares are.
    role: Role,
    /// The directory Rust looks in for the files of its `mod x;`.
    nested: &'p Path,
}

/// The module tree of a crate's targets as it is found, by path relative to `root`.
struct Tree<'a> {
    root: &'a Path,
    found: BTreeMap<PathBuf, Module>,
}

impl Tree<'_> {
    /// Reads the module at `spot` and every module it declares.
    fn file(&mut self, spot: Spot<'_>) -> Result<(), Error> {
        let Spot { path, role, nested } = spot;
        if self.found.contains_key(path) {
            return Ok(());
        }
        let full = self.root.join(path);
        let text = std::fs::read_to_string(&full).map_err(|source| Error::Read { path: full, source })?;
        let syntax = syn::parse_file(&text).map_err(|source| Error::Parse { path: path.to_path_buf(), source })?;
        let generated = text.strip_prefix("// ").is_some_and(|rest| rest.starts_with(Origin::MARK));
        let role = if generated { Role::Generated } else { role };
        self.items(&syntax.items, Spot { path, role, nested })?;
        self.found.insert(path.to_path_buf(), Module { path: path.to_path_buf(), syntax, role });
        Ok(())
    }

    /// Follows every `mod` among `items` of the file at `spot`.
    fn items(&mut self, items: &[syn::Item], spot: Spot<'_>) -> Result<(), Error> {
        let Spot { path, role, nested } = spot;
        for item in items {
            let syn::Item::Mod(module) = item else { continue };
            let role = if role == Role::Test || module.attrs.is_test() { Role::Test } else { Role::Inner };
            let name = module.ident.to_string();
            if let Some((_, inner)) = &module.content {
                self.items(inner, Spot { path, role, nested: &nested.join(&name) })?;
                continue;
            }
            let here = path.parent().unwrap_or(Path::new(""));
            let candidates = match module.attrs.place() {
                Some(place) => vec![here.join(place)],
                None => vec![nested.join(format!("{name}.rs")), nested.join(&name).join("mod.rs")],
            };
            let Some(child) = candidates.into_iter().find(|c| self.root.join(c).is_file()) else {
                return Err(Error::Module { path: path.to_path_buf(), name });
            };
            // `x/mod.rs` and a `#[path]` file nest in their own directory, `x.rs` in `x/`
            let named = module.attrs.place().is_none() && child.file_name().is_some_and(|f| f != "mod.rs");
            let dir = child.parent().unwrap_or(Path::new("")).to_path_buf();
            let below = if named { dir.join(&name) } else { dir };
            self.file(Spot { path: &child, role, nested: &below })?;
        }
        Ok(())
    }
}
