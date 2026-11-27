//! `cargo xtask component`: every guest crate built for `wasm32-unknown-unknown` with the
//! `component` profile, and each core module wrapped by `wit-component` into a component at
//! `target/plugin/<name>.wasm` (decision 0035). The gate runs it before the tests, which
//! load the components, and before [`Sites`], which builds every CLI site fixture through them.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::check::{Cargo, Command, Flag, Step, Target};
use crate::error::Error;
use crate::process::Process;
use crate::report::{Line, Report};

/// The guests, built and wrapped.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Components;

/// One guest crate and the component it becomes.
#[derive(Debug, Clone, Copy)]
struct Guest {
    package: &'static str,
    name: &'static str,
    /// The plugin id `ink` carries the component under; `None` for a guest that is only tested.
    bundled: Option<&'static str>,
}

/// The components `ink` carries, as `target/plugin/bundle.json` lists them: what `uses-ink-cli`'s
/// build script reads to embed them, so the registry is derived from this list and from nothing
/// else (decision 0054).
#[derive(Debug, serde::Serialize)]
struct Bundle {
    schema: &'static str,
    components: Vec<Member>,
}

/// One component of a [`Bundle`].
#[derive(Debug, serde::Serialize)]
struct Member {
    id: &'static str,
    /// Its file in the same folder.
    file: String,
    version: &'static str,
    /// The BLAKE3 of the file, in hex.
    hash: String,
}

impl Components {
    /// Where the components are written, relative to the root.
    pub(crate) const DIR: &'static str = "target/plugin";

    const GUESTS: [Guest; 5] = [
        Guest {
            package: "uses-ink-format-markdown-guest",
            name: "format-markdown",
            bundled: Some("ink:format-markdown"),
        },
        Guest {
            package: "uses-ink-format-obsidian-guest",
            name: "format-obsidian",
            bundled: Some("ink:format-obsidian"),
        },
        Guest { package: "uses-ink-theme-quiet-guest", name: "theme-quiet", bundled: Some("ink:theme-quiet") },
        Guest { package: "uses-ink-source-git-guest", name: "source-git", bundled: Some("ink:source-git") },
        Guest { package: "uses-ink-host-wasmtime-hostile", name: "hostile", bundled: None },
    ];

    const BUILD: Cargo<[&'static str; 5]> = Cargo {
        command: Command::Build,
        scope: [
            Self::GUESTS[0].package,
            Self::GUESTS[1].package,
            Self::GUESTS[2].package,
            Self::GUESTS[3].package,
            Self::GUESTS[4].package,
        ],
        flags: &[Flag::Component],
        target: Target::Browser,
    };
}

impl Guest {
    /// The core module cargo writes for it.
    fn module(self, root: &Path) -> PathBuf {
        root.join("target/wasm32-unknown-unknown/component").join(format!("{}.wasm", self.package.replace('-', "_")))
    }

    /// Wraps its core module into a component, returning where it was written and, when `ink`
    /// carries it, its entry in the bundle.
    fn wrap(self, root: &Path) -> Result<(PathBuf, Option<Member>), Error> {
        let module = self.module(root);
        let bytes = std::fs::read(&module).map_err(|source| Error::Read { path: module.clone(), source })?;
        let component = wit_component::ComponentEncoder::default()
            .validate(true)
            .module(&bytes)
            .and_then(wit_component::ComponentEncoder::encode)
            .map_err(|source| Error::Wrap { path: module, source })?;
        let relative = Path::new(Components::DIR).join(format!("{}.wasm", self.name));
        let path = root.join(&relative);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|source| Error::Write { path: dir.to_owned(), source })?;
        }
        let hash = blake3::hash(&component).to_hex().to_string();
        std::fs::write(&path, component).map_err(|source| Error::Write { path, source })?;
        let member = self.bundled.map(|id| Member {
            id,
            file: format!("{}.wasm", self.name),
            version: env!("CARGO_PKG_VERSION"),
            hash,
        });
        Ok((relative, member))
    }
}

impl Bundle {
    /// The file it is written to, relative to the root.
    const FILE: &'static str = "target/plugin/bundle.json";

    fn write(&self, root: &Path) -> Result<PathBuf, Error> {
        let relative = PathBuf::from(Self::FILE);
        let path = root.join(&relative);
        let text = serde_json::to_vec_pretty(self).map_err(|source| Error::Bundle { path: path.clone(), source })?;
        std::fs::write(&path, text).map_err(|source| Error::Write { path, source })?;
        Ok(relative)
    }
}

impl Step for Components {
    fn run(&self, root: &Path, report: &mut Report) {
        let before = report.failures().len();
        Self::BUILD.run(root, report);
        if report.failures().len() > before {
            return;
        }
        let mut components = Vec::new();
        for guest in Self::GUESTS {
            match guest.wrap(root) {
                Ok((path, member)) => {
                    report.line(Line::Wrote(path));
                    components.extend(member);
                }
                Err(error) => report.fail(error),
            }
        }
        if report.failures().len() > before {
            return;
        }
        match (Bundle { schema: "uses.bundle/v1", components }).write(root) {
            Ok(path) => report.line(Line::Wrote(path)),
            Err(error) => report.fail(error),
        }
    }
}

impl fmt::Display for Components {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}, then each wrapped into {} with a bundle.json for `ink` to embed", Self::BUILD, Self::DIR)
    }
}

/// What `ink build --check --json` says: the deploy each way, or none when the site did not
/// render (a fixture may fail on purpose, the same way both times).
#[derive(Debug, serde::Deserialize)]
struct Compared {
    native: Option<String>,
    component: Option<String>,
}

/// `ink build --check` on every CLI site fixture: each builds the same deploy natively and
/// through the components `ink` carries, so this is also the test that it carries them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Sites;

impl Sites {
    /// The site fixtures, relative to the root.
    const DIR: &'static str = "cli/fixture/site";

    /// The cache their builds share, relative to the root.
    const CACHE: &'static str = "target/ink-cache";

    const BUILD: Cargo<[&'static str; 1]> =
        Cargo { command: Command::Build, scope: ["uses-ink-cli"], flags: &[], target: Target::Host };

    /// Every fixture folder, sorted.
    fn fixtures(root: &Path) -> Result<Vec<PathBuf>, Error> {
        let dir = root.join(Self::DIR);
        let entries = std::fs::read_dir(&dir).map_err(|source| Error::Read { path: dir.clone(), source })?;
        let mut fixtures = Vec::new();
        for entry in entries {
            let path = entry.map_err(|source| Error::Read { path: dir.clone(), source })?.path();
            if path.is_dir() {
                fixtures.push(path);
            }
        }
        fixtures.sort();
        Ok(fixtures)
    }
}

impl Step for Sites {
    fn run(&self, root: &Path, report: &mut Report) {
        let before = report.failures().len();
        Self::BUILD.run(root, report);
        if report.failures().len() > before {
            return;
        }
        let fixtures = match Self::fixtures(root) {
            Ok(fixtures) => fixtures,
            Err(error) => return report.fail(error),
        };
        let ink = root.join("target/debug/ink");
        for fixture in fixtures {
            let mut command = std::process::Command::new(&ink);
            command
                .args(["--json", "build", "--check"])
                .arg(&fixture)
                // kept under target/, never in the fixture
                .arg("--cache")
                .arg(root.join(Self::CACHE));
            let path = fixture.strip_prefix(root).unwrap_or(&fixture).to_owned();
            match Process::from(command).answer::<Compared>() {
                Ok(compared) if compared.native == compared.component => report.line(Line::Matched(path)),
                Ok(compared) => {
                    report.fail(Error::Mismatch { path, native: compared.native, component: compared.component });
                }
                Err(source) => report.fail(Error::Site { path, source }),
            }
        }
    }
}

impl fmt::Display for Sites {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}, then `ink build --check` on every folder of {}", Self::BUILD, Self::DIR)
    }
}
