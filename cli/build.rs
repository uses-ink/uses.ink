//! Embeds the first-party components in `ink` (decision 0054).
//!
//! `cargo xtask component` writes the components and a `bundle.json` listing them (id, file,
//! version, BLAKE3) into `target/plugin/`, or into the folder `INK_COMPONENTS` names. This reads
//! that list, checks every file against its hash and writes the registry `src/bundle.rs`
//! includes, so the set `ink` carries is the set xtask built and no list of it is kept here.
//!
//! A release-profile build (`cargo install`, `dist`, the image) with no list fails with the
//! command to run. A debug build with none carries nothing and says so when a component is asked
//! for (`Error::Unbundled`), so `cargo check` and the lint stage need no wasm build.

use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The list xtask wrote.
#[derive(Debug, Deserialize)]
struct Bundle {
    schema: String,
    components: Vec<Member>,
}

/// One component of it.
#[derive(Debug, Deserialize)]
struct Member {
    id: String,
    file: String,
    version: String,
    hash: String,
}

/// Why the registry could not be made.
#[derive(Debug)]
enum Failure {
    /// A release build found no list.
    Unbuilt {
        dir: PathBuf,
    },
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    /// The list is of a schema this script does not read.
    Schema {
        path: PathBuf,
        found: String,
    },
    /// A file is not the one the list hashed (stale or torn).
    Changed {
        path: PathBuf,
        expected: String,
        found: String,
    },
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A file is not named as `Components` finds it from the id: `ink:<name>` is `<name>.wasm`.
    Named {
        id: String,
        file: String,
    },
}

impl fmt::Display for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unbuilt { dir } => write!(
                out,
                "ink carries its components and {} holds none: run `cargo xtask component` first (or point INK_COMPONENTS at its output)",
                dir.display()
            ),
            Self::Read { path, source } => write!(out, "cannot read {}: {source}", path.display()),
            Self::Parse { path, source } => write!(out, "{} is not the bundle list: {source}", path.display()),
            Self::Schema { path, found } => {
                write!(out, "{} is schema `{found}`, this build reads `uses.bundle/v1`", path.display())
            }
            Self::Changed { path, expected, found } => write!(
                out,
                "{} hashes to {found}, bundle.json says {expected}: run `cargo xtask component` again",
                path.display()
            ),
            Self::Write { path, source } => write!(out, "cannot write {}: {source}", path.display()),
            Self::Named { id, file } => {
                write!(out, "`{id}` is listed as `{file}`, but a folder holds it as `<name>.wasm` after the colon")
            }
        }
    }
}

/// Where the components are and how strict the build is about them.
#[derive(Debug)]
struct Plan {
    dir: PathBuf,
    release: bool,
    out: PathBuf,
}

impl Plan {
    const LIST: &'static str = "bundle.json";

    fn env() -> Self {
        let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
        let root = manifest.parent().map_or_else(PathBuf::new, Path::to_path_buf);
        let dir = std::env::var_os("INK_COMPONENTS").map_or_else(|| root.join("target/plugin"), PathBuf::from);
        let release = std::env::var("PROFILE").is_ok_and(|profile| profile == "release");
        let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default()).join("bundled.rs");
        Self { dir, release, out }
    }

    /// Tells cargo what to rerun for: the list and every file once there is a list, else the
    /// nearest folder that exists, whose change is the list appearing.
    fn watch(&self, members: &[Member]) {
        println!("cargo::rerun-if-env-changed=INK_COMPONENTS");
        println!("cargo::rerun-if-changed=build.rs");
        let list = self.dir.join(Self::LIST);
        if list.exists() {
            println!("cargo::rerun-if-changed={}", list.display());
            for member in members {
                println!("cargo::rerun-if-changed={}", self.dir.join(&member.file).display());
            }
        } else if self.dir.exists() {
            println!("cargo::rerun-if-changed={}", self.dir.display());
        } else if let Some(parent) = self.dir.parent() {
            println!("cargo::rerun-if-changed={}", parent.display());
        }
    }

    /// The list, or none when there is no file; a release build refuses none.
    fn list(&self) -> Result<Vec<Member>, Failure> {
        let path = self.dir.join(Self::LIST);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(missing) if missing.kind() == std::io::ErrorKind::NotFound && !self.release => return Ok(Vec::new()),
            Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => {
                return Err(Failure::Unbuilt { dir: self.dir.clone() });
            }
            Err(source) => return Err(Failure::Read { path, source }),
        };
        let bundle: Bundle =
            serde_json::from_str(&text).map_err(|source| Failure::Parse { path: path.clone(), source })?;
        if bundle.schema != "uses.bundle/v1" {
            return Err(Failure::Schema { path, found: bundle.schema });
        }
        Ok(bundle.components)
    }

    /// The registry's source: one `Entry` per member, its file embedded.
    fn source(&self, members: &[Member]) -> Result<String, Failure> {
        let mut text = String::from("&[\n");
        for member in members {
            if member.id.rsplit(':').next().map(|name| format!("{name}.wasm")).as_deref() != Some(member.file.as_str())
            {
                return Err(Failure::Named { id: member.id.clone(), file: member.file.clone() });
            }
            let path = self.dir.join(&member.file);
            let bytes = std::fs::read(&path).map_err(|source| Failure::Read { path: path.clone(), source })?;
            let found = blake3::hash(&bytes);
            if found.to_hex().as_str() != member.hash {
                return Err(Failure::Changed {
                    path,
                    expected: member.hash.clone(),
                    found: found.to_hex().to_string(),
                });
            }
            let path = std::fs::canonicalize(&path).map_err(|source| Failure::Read { path, source })?;
            // writing to a String cannot fail
            let _ = writeln!(
                text,
                "    Entry::new({:?}, {:?}, {:?}, include_bytes!({:?})),",
                member.id,
                member.version,
                found.as_bytes(),
                path.to_string_lossy()
            );
        }
        text.push(']');
        Ok(text)
    }

    fn run(&self) -> Result<(), Failure> {
        let members = self.list()?;
        self.watch(&members);
        let source = self.source(&members)?;
        std::fs::write(&self.out, source).map_err(|source| Failure::Write { path: self.out.clone(), source })
    }
}

fn main() {
    let plan = Plan::env();
    if let Err(failure) = plan.run() {
        // watch even on failure, so fixing it reruns the script
        plan.watch(&[]);
        println!("cargo::error={failure}");
        // an empty registry keeps the crate's own errors out of the way of this one
        let _ = std::fs::write(Path::new(&plan.out), "&[]");
    }
}
