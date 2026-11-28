//! `ink build --check` on every site fixture.

use std::fmt;
use std::path::{Path, PathBuf};

use crate::check::{Cargo, Command, Step, Target};
use crate::error::Error;
use crate::process::Process;
use crate::report::{Line, Report};

/// What `ink build --check --json` says: the deploy each way, or none when the site did not
/// render (a fixture may fail on purpose, the same way both times).
#[derive(Debug, serde::Deserialize)]
struct Compared {
    native: Option<String>,
    component: Option<String>,
    errors: usize,
    warnings: usize,
}

/// `ink build --check` on every CLI site fixture: each builds the same deploy natively and
/// through the components `ink` carries, so this is also the test that it carries them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Sites;

impl Sites {
    /// The site fixtures, relative to the root.
    const DIR: &'static str = "crates/surface/cli/fixture/site";

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
                .args(["--json", "-C"])
                .arg(&fixture)
                .args(["build", "--check"])
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
        Handbook.run(root, report);
    }
}

impl fmt::Display for Sites {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "{}, then `ink build --check` on every folder of {} and on {}",
            Self::BUILD,
            Self::DIR,
            Handbook::DIR
        )
    }
}

/// The documentation site, `docs/` built by `ink` (docs/site.md, decision 0120): `ink build
/// --check` twice, each with a cache of its own, which must say nothing (no error, no warning, so
/// no broken link), build the same deploy natively and as components, and build the same deploy
/// both times.
#[derive(Debug, Clone, Copy)]
struct Handbook;

impl Handbook {
    /// The site's folder, relative to the root.
    const DIR: &'static str = "docs";

    /// The two caches, relative to the root, emptied before each build.
    const CACHES: [&'static str; 2] = ["target/ink-docs/first", "target/ink-docs/second"];

    /// An empty folder at `cache` under `root`.
    fn emptied(root: &Path, cache: &str) -> Result<PathBuf, Error> {
        let path = root.join(cache);
        match std::fs::remove_dir_all(&path) {
            Ok(()) => Ok(path),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(path),
            Err(source) => Err(Error::Write { path, source }),
        }
    }

    /// `ink build --check` of the site with `cache`, its diagnostics on stderr.
    fn built(root: &Path, cache: &Path) -> Result<Compared, Error> {
        let mut command = std::process::Command::new(root.join("target/debug/ink"));
        command
            .args(["--json", "-C"])
            .arg(root.join(Self::DIR))
            .args(["build", "--check", "--source", "folder", "--cache"])
            .arg(cache);
        Process::from(command).answer().map_err(|source| Error::Site { path: Self::DIR.into(), source })
    }

    /// One build, refused unless it is clean and its two deploys agree.
    fn once(root: &Path, cache: &str) -> Result<Option<String>, Error> {
        let path = PathBuf::from(Self::DIR);
        let compared = Self::built(root, &Self::emptied(root, cache)?)?;
        if compared.errors > 0 || compared.warnings > 0 {
            return Err(Error::Unclean { path, errors: compared.errors, warnings: compared.warnings });
        }
        if compared.native != compared.component {
            return Err(Error::Mismatch { path, native: compared.native, component: compared.component });
        }
        Ok(compared.native)
    }
}

impl Step for Handbook {
    fn run(&self, root: &Path, report: &mut Report) {
        let path = PathBuf::from(Self::DIR);
        let mut deploys = Vec::new();
        for cache in Self::CACHES {
            match Self::once(root, cache) {
                Ok(deploy) => deploys.push(deploy),
                Err(error) => return report.fail(error),
            }
        }
        match deploys.as_slice() {
            [first, second] if first == second => report.line(Line::Reproduced(path)),
            [first, second] => report.fail(Error::Unstable { path, first: first.clone(), second: second.clone() }),
            _ => {}
        }
    }
}

impl fmt::Display for Handbook {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`ink build --check` of {} twice, with no diagnostic and the same deploy", Self::DIR)
    }
}
