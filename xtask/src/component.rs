//! `cargo xtask component`: every guest crate built for `wasm32-unknown-unknown` with the
//! `component` profile, and each core module wrapped by `wit-component` into a component at
//! `target/plugin/<name>.wasm` (decision 0035). The gate runs it before the tests, which
//! load the components, and before [`Sites`], which builds every CLI site fixture through them.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::check::{Cargo, Command, Flag, Step, Target};
use crate::error::Error;
use crate::output::{Line, Report};
use crate::process::Process;

/// The guests, built and wrapped.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Components;

/// One guest crate and the component it becomes.
#[derive(Debug, Clone, Copy)]
struct Guest {
    package: &'static str,
    name: &'static str,
}

impl Components {
    /// Where the components are written, relative to the root.
    pub(crate) const DIR: &'static str = "target/plugin";

    const GUESTS: [Guest; 3] = [
        Guest { package: "uses-ink-format-markdown-guest", name: "format-markdown" },
        Guest { package: "uses-ink-theme-quiet-guest", name: "theme-quiet" },
        Guest { package: "uses-ink-host-wasmtime-hostile", name: "hostile" },
    ];

    const BUILD: Cargo<[&'static str; 3]> = Cargo {
        command: Command::Build,
        scope: [Self::GUESTS[0].package, Self::GUESTS[1].package, Self::GUESTS[2].package],
        flags: &[Flag::Component],
        target: Target::Browser,
    };
}

impl Guest {
    /// The core module cargo writes for it.
    fn module(self, root: &Path) -> PathBuf {
        root.join("target/wasm32-unknown-unknown/component").join(format!("{}.wasm", self.package.replace('-', "_")))
    }

    /// Wraps its core module into a component, returning where it was written.
    fn wrap(self, root: &Path) -> Result<PathBuf, Error> {
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
        std::fs::write(&path, component).map_err(|source| Error::Write { path, source })?;
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
        for guest in Self::GUESTS {
            match guest.wrap(root) {
                Ok(path) => report.line(Line::Wrote(path)),
                Err(error) => report.fail(error),
            }
        }
    }
}

impl fmt::Display for Components {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}, then each wrapped into {}", Self::BUILD, Self::DIR)
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
/// through the components.
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
                .arg("--components")
                .arg(root.join(Components::DIR))
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
